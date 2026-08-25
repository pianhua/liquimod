param(
    [switch]$SkipFrontend = $false,
    [string]$OutputRoot = ""
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
$liveDistRoot = [IO.Path]::GetFullPath((Join-Path $rootDir "dist")).TrimEnd('\', '/')
if ([string]::IsNullOrWhiteSpace($OutputRoot)) {
    $runId = [guid]::NewGuid().ToString("N")
    $distRoot = Join-Path $rootDir "dist-staging\run-$runId"
} else {
    $distRoot = [IO.Path]::GetFullPath($OutputRoot).TrimEnd('\', '/')
}

if ($distRoot.Equals($liveDistRoot, [StringComparison]::OrdinalIgnoreCase) -or
    $distRoot.StartsWith("$liveDistRoot\", [StringComparison]::OrdinalIgnoreCase)) {
    throw "OutputRoot must not be the live dist directory: $distRoot"
}
if (Test-Path -LiteralPath $distRoot) {
    throw "OutputRoot already exists; choose a fresh staging directory: $distRoot"
}

$distDir = Join-Path $distRoot "LiquiMod-Windows-x64"
New-Item -ItemType Directory -Force -Path $distDir | Out-Null

$mainBinary = "$rootDir\target\release\XXMI Launcher.exe"
$helperBinary = "$rootDir\target\release\liquimod-refresh-helper.exe"
if (-not (Test-Path $mainBinary)) {
    throw "Main application binary was not produced: $mainBinary"
}
if (-not (Test-Path $helperBinary)) {
    throw "Refresh helper binary was not produced: $helperBinary"
}
Copy-Item $mainBinary -Destination "$distDir\XXMI Launcher.exe"
Copy-Item $helperBinary -Destination $distDir
Copy-Item "$rootDir\LICENSE" -Destination $distDir
Copy-Item "$rootDir\README.md" -Destination $distDir

$builtinPackages = "$rootDir\assets\builtin-core\Packages"
if (-not (Test-Path $builtinPackages)) {
    throw "Bundled SRMI/XXMI packages are missing: $builtinPackages"
}
foreach ($requiredPackageFile in @(
    "SRMI\Manifest.json",
    "SRMI\d3dx.ini",
    "XXMI\Manifest.json",
    "XXMI\3dmloader.dll",
    "XXMI\d3d11.dll"
)) {
    if (-not (Test-Path (Join-Path $builtinPackages $requiredPackageFile))) {
        throw "Required bundled runtime file is missing: $requiredPackageFile"
    }
}
Copy-Item $builtinPackages -Destination "$distDir\Packages" -Recurse -Force

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
