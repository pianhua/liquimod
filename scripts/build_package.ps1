<#
.SYNOPSIS
Builds and assembles a versioned LiquiMod Windows release without deleting existing dist contents.

.EXAMPLE
.\scripts\build_package.ps1 -Version 0.6.1

.EXAMPLE
.\scripts\build_package.ps1 -SkipFrontend
#>
[CmdletBinding()]
param(
    [switch]$SkipFrontend,
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"

$rootDir = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
Set-Location $rootDir

$packageJson = Get-Content -LiteralPath (Join-Path $rootDir "app\package.json") -Raw | ConvertFrom-Json
$versionText = if ([string]::IsNullOrWhiteSpace($Version)) { [string]$packageJson.version } else { $Version -replace '^[vV]', '' }
if ($versionText -notmatch '^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$') {
    throw "Version must be a semantic version such as 0.6.1; received: $versionText"
}

$releaseVersion = "v$versionText"
$distRoot = Join-Path $rootDir "dist"
$releaseRoot = Join-Path $distRoot "releases\$releaseVersion"
$stagingRoot = Join-Path $distRoot ".staging\LiquiMod-Windows-x64-$releaseVersion-$([Guid]::NewGuid().ToString('N'))"
$stagingPackage = Join-Path $stagingRoot "LiquiMod-Windows-x64"
$finalPackage = Join-Path $releaseRoot "LiquiMod-Windows-x64"

function Remove-GeneratedPackage([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return }
    $marker = Join-Path $Path ".liquimod-generated"
    if (-not (Test-Path -LiteralPath $marker)) {
        throw "Refusing to replace a package directory without LiquiMod marker: $Path"
    }
    [System.IO.Directory]::Delete($Path, $true)
}

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "  LiquiMod Local Packaging Pipeline" -ForegroundColor Cyan
Write-Host "  Version: $releaseVersion" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan

New-Item -ItemType Directory -Force -Path $stagingPackage | Out-Null

try {
    if (-not $SkipFrontend) {
        Write-Host "`n[1/4] Building frontend assets..." -ForegroundColor Yellow
        Push-Location (Join-Path $rootDir "app")
        npm run build
        Pop-Location
    } else {
        if (-not (Test-Path -LiteralPath (Join-Path $rootDir "app\build"))) {
            throw "-SkipFrontend was used but app/build does not exist. Run npm run build first."
        }
        Write-Host "`n[1/4] Skipping frontend build (using existing build dir)" -ForegroundColor DarkGray
    }

    Write-Host "`n[2/4] Compiling F10 refresh helper..." -ForegroundColor Yellow
    cargo build --release -p liquimod-refresh-helper

    Write-Host "`n[3/4] Compiling LiquiMod App..." -ForegroundColor Yellow
    cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml

    Write-Host "`n[3.5/4] Building NSIS installer..." -ForegroundColor Yellow
    Push-Location (Join-Path $rootDir "app")
    npx --no-install tauri build --bundles nsis
    Pop-Location

    Write-Host "`n[4/4] Assembling versioned release package..." -ForegroundColor Yellow
    $mainBinary = Join-Path $rootDir "target\release\XXMI Launcher.exe"
    if (-not (Test-Path -LiteralPath $mainBinary)) {
        $legacyMainBinary = Join-Path $rootDir "target\release\liquimod-app.exe"
        if (-not (Test-Path -LiteralPath $legacyMainBinary)) {
            throw "Main application binary was not produced: $mainBinary"
        }
        $mainBinary = $legacyMainBinary
    }

    Copy-Item -LiteralPath $mainBinary -Destination (Join-Path $stagingPackage "XXMI Launcher.exe")
    Copy-Item -LiteralPath (Join-Path $rootDir "target\release\liquimod-refresh-helper.exe") -Destination $stagingPackage
    Copy-Item -LiteralPath (Join-Path $rootDir "LICENSE") -Destination $stagingPackage
    Copy-Item -LiteralPath (Join-Path $rootDir "README.md") -Destination $stagingPackage

    $builtinPackages = Join-Path $rootDir "assets\builtin-core\Packages"
    if (-not (Test-Path -LiteralPath $builtinPackages)) {
        throw "Bundled SRMI/XXMI packages are missing: $builtinPackages"
    }
    Copy-Item -LiteralPath $builtinPackages -Destination (Join-Path $stagingPackage "Packages") -Recurse -Force

    $zipPath = Join-Path $stagingRoot "LiquiMod-Windows-x64.zip"
    Compress-Archive -Path (Join-Path $stagingPackage "*") -DestinationPath $zipPath -Force
    $hash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash

    $installer = Get-ChildItem -Path (Join-Path $rootDir "target"), (Join-Path $rootDir "app\src-tauri\target") -Filter "*-setup.exe" -File -Recurse -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $installer) { throw "NSIS installer was not produced" }
    $installerPath = Join-Path $stagingRoot "LiquiMod-Windows-x64-setup.exe"
    Copy-Item -LiteralPath $installer.FullName -Destination $installerPath -Force
    $installerHash = (Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash

    New-Item -ItemType Directory -Force -Path $releaseRoot | Out-Null
    Remove-GeneratedPackage $finalPackage
    Move-Item -LiteralPath $stagingPackage -Destination $finalPackage
    [System.IO.File]::WriteAllText((Join-Path $finalPackage ".liquimod-generated"), "Generated by scripts/build_package.ps1 for $releaseVersion`r`n")
    Copy-Item -LiteralPath $zipPath -Destination (Join-Path $releaseRoot "LiquiMod-Windows-x64.zip") -Force
    Copy-Item -LiteralPath $installerPath -Destination (Join-Path $releaseRoot "LiquiMod-Windows-x64-setup.exe") -Force
    "$hash  LiquiMod-Windows-x64.zip`r`n$installerHash  LiquiMod-Windows-x64-setup.exe`r`n" |
        Out-File -LiteralPath (Join-Path $releaseRoot "SHA256SUMS.txt") -Encoding utf8

    $zipSizeMB = [math]::Round((Get-Item -LiteralPath (Join-Path $releaseRoot "LiquiMod-Windows-x64.zip")).Length / 1MB, 2)
    Write-Host "`n==========================================" -ForegroundColor Green
    Write-Host "  Packaging completed successfully!" -ForegroundColor Green
    Write-Host "  Release folder: $releaseRoot" -ForegroundColor Green
    Write-Host "  Zip archive   : $(Join-Path $releaseRoot 'LiquiMod-Windows-x64.zip') ($zipSizeMB MB)" -ForegroundColor Green
    Write-Host "  Installer     : $(Join-Path $releaseRoot 'LiquiMod-Windows-x64-setup.exe')" -ForegroundColor Green
    Write-Host "==========================================" -ForegroundColor Green
}
finally {
    if (Test-Path -LiteralPath $stagingRoot) {
        [System.IO.Directory]::Delete($stagingRoot, $true)
    }
    $stagingParent = Join-Path $distRoot ".staging"
    if ((Test-Path -LiteralPath $stagingParent) -and (@(Get-ChildItem -LiteralPath $stagingParent -Force).Count -eq 0)) {
        [System.IO.Directory]::Delete($stagingParent, $false)
    }
}
