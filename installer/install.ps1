param(
    [switch]$NoAutostart,
    [switch]$NoLaunch
)

$ErrorActionPreference = "Stop"

$repo = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $repo "target\release\notch.exe"
if (-not (Test-Path $exe)) {
    $exe = Join-Path $PSScriptRoot "notch.exe"
}
if (-not (Test-Path $exe)) {
    throw "notch.exe not found. Run 'cargo build --release' or place notch.exe next to install.ps1."
}

Get-Process notch -ErrorAction SilentlyContinue | Stop-Process -Force

$dir = Join-Path $env:LOCALAPPDATA "Notch"
New-Item -ItemType Directory -Force $dir | Out-Null
$target = Join-Path $dir "notch.exe"
Copy-Item $exe $target -Force

$programs = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
New-Item -ItemType Directory -Force $programs | Out-Null
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut((Join-Path $programs "NOTCH.lnk"))
$shortcut.TargetPath = $target
$shortcut.WorkingDirectory = $dir
$shortcut.Description = "NOTCH - dynamic notch for Windows"
$shortcut.Save()

if (-not $NoAutostart) {
    $run = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
    if (-not (Test-Path $run)) {
        New-Item -Path $run -Force | Out-Null
    }
    Set-ItemProperty -Path $run -Name "Notch" -Value ('"' + $target + '"')
}

if (-not $NoLaunch) {
    Start-Process $target
}

Write-Host "NOTCH installed at $dir"
Write-Host "Start Menu shortcut created. Uninstall with uninstall.ps1."
