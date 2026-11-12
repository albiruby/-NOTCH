param(
    [string]$Version = "1.2.0"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $cargo = (Get-Command cargo -ErrorAction SilentlyContinue).Source
    if (-not $cargo) {
        $cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
    }
    & $cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    $dist = Join-Path $root "dist\NOTCH-$Version"
    Remove-Item $dist -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $dist | Out-Null

    Copy-Item "target\release\notch.exe" $dist
    Copy-Item "installer\install.ps1" $dist
    Copy-Item "installer\uninstall.ps1" $dist
    Copy-Item "README.md" $dist

    $zip = Join-Path $root "dist\NOTCH-$Version.zip"
    Remove-Item $zip -Force -ErrorAction SilentlyContinue
    Compress-Archive -Path (Join-Path $dist "*") -DestinationPath $zip

    Write-Host "Packaged $zip"
} finally {
    Pop-Location
}
