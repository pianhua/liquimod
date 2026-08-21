param(
    [switch]$SkipFrontend = $false
)

$ErrorActionPreference = "Stop"

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "  LiquiMod Local Packaging Pipeline" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan

$rootDir = Split-Path -Parent $PSScriptRoot
Set-Location $rootDir

# 1. Frontend Build
if (-not $SkipFrontend) {
    Write-Host "`n[1/4] Building frontend assets..." -ForegroundColor Yellow
    Set-Location "$rootDir\app"
    npm run build
    Set-Location $rootDir
} else {
    Write-Host "`n[1/4] Skipping frontend build (using existing build dir)" -ForegroundColor DarkGray
}

# 2. Build Helper
Write-Host "`n[2/4] Compiling F10 refresh helper (liquimod-refresh-helper)..." -ForegroundColor Yellow
cargo build --release -p liquimod-refresh-helper

# 3. Build Main App
Write-Host "`n[3/4] Compiling LiquiMod App (with embedded frontend)..." -ForegroundColor Yellow
cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml

Write-Host "`n[3.5/4] Building NSIS installer..." -ForegroundColor Yellow
Set-Location "$rootDir\app"
npx --no-install tauri build --bundles nsis
Set-Location $rootDir

# 4. Assemble Dist Package
Write-Host "`n[4/4] Assembling release package..." -ForegroundColor Yellow
$distDir = "$rootDir\dist\LiquiMod-Windows-x64"
$distRoot = "$rootDir\dist"

if (Test-Path $distRoot) {
    Remove-Item -Recurse -Force $distRoot
}
New-Item -ItemType Directory -Force -Path $distDir | Out-Null

Copy-Item "$rootDir\target\release\liquimod-app.exe" -Destination $distDir
Copy-Item "$rootDir\target\release\liquimod-refresh-helper.exe" -Destination $distDir
Copy-Item "$rootDir\LICENSE" -Destination $distDir
Copy-Item "$rootDir\README.md" -Destination $distDir

$zipPath = "$distRoot\LiquiMod-Windows-x64.zip"
Compress-Archive -Path "$distDir\*" -DestinationPath $zipPath -Force

$hash = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash
"$hash  LiquiMod-Windows-x64.zip" | Out-File "$distRoot\SHA256SUMS.txt" -Encoding utf8

$installer = Get-ChildItem -Path "$rootDir\target", "$rootDir\app\src-tauri\target" -Filter "*-setup.exe" -File -Recurse -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $installer) { throw "NSIS installer was not produced" }
$installerPath = "$distRoot\LiquiMod-Windows-x64-setup.exe"
Copy-Item $installer.FullName -Destination $installerPath -Force
$installerHash = (Get-FileHash -Path $installerPath -Algorithm SHA256).Hash
Add-Content "$distRoot\SHA256SUMS.txt" "$installerHash  LiquiMod-Windows-x64-setup.exe"

$zipSizeMB = [math]::Round((Get-Item $zipPath).Length / 1MB, 2)

Write-Host "`n==========================================" -ForegroundColor Green
Write-Host "  Packaging completed successfully!" -ForegroundColor Green
Write-Host "  Package folder: $distDir" -ForegroundColor Green
Write-Host "  Zip archive   : $zipPath ($zipSizeMB MB)" -ForegroundColor Green
Write-Host "  SHA256        : $hash" -ForegroundColor Green
Write-Host "  Installer     : $installerPath" -ForegroundColor Green
Write-Host "==========================================" -ForegroundColor Green
