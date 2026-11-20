# Builds the agent and the new Rust installer, and puts the latest installer and uninstaller in
# dist\ for local install/testing:
#   dist\vitals-setup.exe      install, update, repair, change settings
#   dist\vitals-uninstall.exe  the same program; a name containing "uninstall" opens the uninstall
#                              screen (installer/setup/src/main.rs, hello)
#
#   powershell -File scripts\build-dist.ps1
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

Write-Host "Building the agent..."
cargo build --release --manifest-path "$root\agent\Cargo.toml"
if ($LASTEXITCODE -ne 0) { throw "agent build failed" }

# The setup embeds agent\target\release\vitals.exe, so it is built second.
Write-Host "Building the installer..."
cargo build --release --manifest-path "$root\installer\setup\Cargo.toml"
if ($LASTEXITCODE -ne 0) { throw "installer build failed" }

$dist = Join-Path $root "dist"
New-Item -ItemType Directory -Force $dist | Out-Null
$setup = "$root\installer\setup\target\release\vitals-setup.exe"
Copy-Item $setup "$dist\vitals-setup.exe" -Force
Copy-Item $setup "$dist\vitals-uninstall.exe" -Force
Get-ChildItem $dist\*.exe | Format-Table Name, Length, LastWriteTime -AutoSize
