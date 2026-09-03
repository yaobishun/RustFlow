[CmdletBinding()]
param(
    [switch]$SkipInstall
)

$ErrorActionPreference = 'Stop'
$projectRoot = $PSScriptRoot
$backendDir = Join-Path $projectRoot 'backend'
$frontendDir = Join-Path $projectRoot 'frontend'
$runDir = Join-Path $projectRoot '.run'

if (-not (Test-Path -LiteralPath (Join-Path $backendDir 'Cargo.toml'))) {
    throw 'backend/Cargo.toml was not found.'
}

if (-not (Test-Path -LiteralPath (Join-Path $frontendDir 'package.json'))) {
    throw 'frontend/package.json was not found.'
}

foreach ($commandName in @('cargo', 'npm')) {
    if (-not (Get-Command $commandName -ErrorAction SilentlyContinue)) {
        throw "$commandName was not found. Install the required development tools first."
    }
}

New-Item -ItemType Directory -Path $runDir -Force | Out-Null

$backendPidFile = Join-Path $runDir 'backend.pid'
$frontendPidFile = Join-Path $runDir 'frontend.pid'

foreach ($pidFile in @($backendPidFile, $frontendPidFile)) {
    if (Test-Path -LiteralPath $pidFile) {
        $savedPid = Get-Content -LiteralPath $pidFile -ErrorAction SilentlyContinue
        if ($savedPid -and (Get-Process -Id $savedPid -ErrorAction SilentlyContinue)) {
            throw "RustFlow is already running (PID $savedPid). Run ./stop.ps1 first."
        }
        Remove-Item -LiteralPath $pidFile -Force
    }
}

if (-not $SkipInstall -and -not (Test-Path -LiteralPath (Join-Path $frontendDir 'node_modules'))) {
    Write-Host 'First run: installing frontend dependencies...'
    & npm --prefix $frontendDir install
    if ($LASTEXITCODE -ne 0) {
        throw 'Frontend dependency installation failed.'
    }
}

$backendOut = Join-Path $runDir 'backend.out.log'
$backendErr = Join-Path $runDir 'backend.err.log'
$frontendOut = Join-Path $runDir 'frontend.out.log'
$frontendErr = Join-Path $runDir 'frontend.err.log'

$env:BIND_ADDR = '127.0.0.1:3100'

$backend = Start-Process -FilePath 'cargo' `
    -ArgumentList @('run', '--manifest-path', (Join-Path $backendDir 'Cargo.toml')) `
    -WorkingDirectory $backendDir `
    -RedirectStandardOutput $backendOut `
    -RedirectStandardError $backendErr `
    -WindowStyle Hidden `
    -PassThru

$frontend = Start-Process -FilePath 'npm.cmd' `
    -ArgumentList @('--prefix', $frontendDir, 'run', 'dev', '--', '--host', '127.0.0.1') `
    -WorkingDirectory $frontendDir `
    -RedirectStandardOutput $frontendOut `
    -RedirectStandardError $frontendErr `
    -WindowStyle Hidden `
    -PassThru

Set-Content -LiteralPath $backendPidFile -Value $backend.Id
Set-Content -LiteralPath $frontendPidFile -Value $frontend.Id

Write-Host "Backend started: PID $($backend.Id), logs: $backendOut / $backendErr"
Write-Host "Frontend started: PID $($frontend.Id), logs: $frontendOut / $frontendErr"
Write-Host 'Open: http://127.0.0.1:5173'
Write-Host 'Stop services: ./stop.ps1'
