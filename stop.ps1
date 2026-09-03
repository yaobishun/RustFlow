[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$runDir = Join-Path $PSScriptRoot '.run'

function Stop-RustFlowProcessTree {
    param([int]$RootProcessId)

    $children = Get-CimInstance Win32_Process -Filter "ParentProcessId = $RootProcessId" -ErrorAction SilentlyContinue
    foreach ($child in $children) {
        Stop-RustFlowProcessTree -RootProcessId $child.ProcessId
    }

    $target = Get-Process -Id $RootProcessId -ErrorAction SilentlyContinue
    if ($target) {
        Stop-Process -Id $RootProcessId -Force -ErrorAction SilentlyContinue
    }
}

foreach ($serviceName in @('backend', 'frontend')) {
    $pidFile = Join-Path $runDir "$serviceName.pid"
    if (-not (Test-Path -LiteralPath $pidFile)) {
        Write-Host "${serviceName}: PID file not found; skipped."
        continue
    }

    $savedPid = Get-Content -LiteralPath $pidFile -ErrorAction SilentlyContinue
    $process = if ($savedPid) { Get-Process -Id $savedPid -ErrorAction SilentlyContinue } else { $null }

    if ($process) {
        Stop-RustFlowProcessTree -RootProcessId $process.Id
        Write-Host "${serviceName}: stopped PID $($process.Id)."
    }
    else {
        Write-Host "${serviceName}: process is not running."
    }

    Remove-Item -LiteralPath $pidFile -Force
}
