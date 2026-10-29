# Builds the release mangue.exe (icon embedded by build.rs, no console window)
# and zips it with the README into target/release/bundle/.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$Version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$Bundle = 'target/release/bundle'
$Stage = "$Bundle/mangue-$Version-windows-x86_64"
$Zip = "$Stage.zip"

Remove-Item -Recurse -Force $Stage, $Zip -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $Stage | Out-Null
Copy-Item target/release/mangue.exe, README.md $Stage
Compress-Archive -Path "$Stage/*" -DestinationPath $Zip

Write-Host "Zip ready: $Root/$Zip"
