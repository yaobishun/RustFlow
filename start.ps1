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

# 完全脱离当前终端启动服务：把命令包进 cmd.exe /c，在子 shell 里重定向
# stdout/stderr 并把 stdin 指向 NUL。这样避免 Start-Process 的
# -RedirectStandardOutput/-RedirectStandardError（Windows PowerShell 5.1 下会让
# 子进程持有当前控制台输入，导致终端启动后卡住、无法继续输入）。
function Start-Detached {
    param(
        [string]$CommandLine,
        [string]$WorkingDirectory,
        [string]$OutFile,
        [string]$ErrFile
    )

    $wrapped = "$CommandLine > `"$OutFile`" 2> `"$ErrFile`" < NUL"
    Start-Process -FilePath 'cmd.exe' `
        -ArgumentList @('/c', $wrapped) `
        -WorkingDirectory $WorkingDirectory `
        -WindowStyle Hidden `
        -PassThru
}

$backendCmd = "cargo run --manifest-path `"$(Join-Path $backendDir 'Cargo.toml')`""
$frontendCmd = "npm.cmd --prefix `"$frontendDir`" run dev -- --host 127.0.0.1"

$backend = Start-Detached -CommandLine $backendCmd -WorkingDirectory $backendDir -OutFile $backendOut -ErrFile $backendErr
$frontend = Start-Detached -CommandLine $frontendCmd -WorkingDirectory $frontendDir -OutFile $frontendOut -ErrFile $frontendErr

Set-Content -LiteralPath $backendPidFile -Value $backend.Id
Set-Content -LiteralPath $frontendPidFile -Value $frontend.Id

Write-Host "Backend started: PID $($backend.Id), logs: $backendOut / $backendErr"
Write-Host "Frontend started: PID $($frontend.Id), logs: $frontendOut / $frontendErr"
Write-Host 'Open: http://127.0.0.1:5173'
Write-Host 'Stop services: ./stop.ps1'
