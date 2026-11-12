param(
    [switch]$KeepSettings
)

$ErrorActionPreference = "Stop"

Get-Process notch -ErrorAction SilentlyContinue | Stop-Process -Force

$run = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
Remove-ItemProperty -Path $run -Name "Notch" -ErrorAction SilentlyContinue

$programs = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
Remove-Item (Join-Path $programs "NOTCH.lnk") -Force -ErrorAction SilentlyContinue

$dir = Join-Path $env:LOCALAPPDATA "Notch"
Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue

if (-not $KeepSettings) {
    Remove-Item (Join-Path $env:APPDATA "Notch") -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "NOTCH removed."
