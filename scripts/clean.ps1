param(
    [switch]$DebugOnly = $false,
    [switch]$All = $false
)

$ErrorActionPreference = "SilentlyContinue"

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "  LiquiMod Workspace Disk Cleanup Tool" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan

$rootDir = Split-Path -Parent $PSScriptRoot
Set-Location $rootDir

function Get-FolderSizeMB ($path) {
    if (-not (Test-Path $path)) { return 0 }
    $size = (Get-ChildItem $path -Recurse -File -Force -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    return [math]::Round($size / 1MB, 2)
}

$initialDebugSize = Get-FolderSizeMB "$rootDir\target\debug"
$initialReleaseSize = Get-FolderSizeMB "$rootDir\target\release"
$initialTotalSize = Get-FolderSizeMB "$rootDir\target"

$debugGB = [math]::Round($initialDebugSize / 1024, 2)
$totalGB = [math]::Round($initialTotalSize / 1024, 2)

Write-Host "`nCurrent target size:" -ForegroundColor Yellow
Write-Host "  * target/debug   (test & debug cache): $initialDebugSize MB ($debugGB GB)"
Write-Host "  * target/release (production build)  : $initialReleaseSize MB"
Write-Host "  * target total                       : $initialTotalSize MB ($totalGB GB)"

if ($All) {
    Write-Host "`n[Mode: Full Clean] Cleaning all build caches..." -ForegroundColor Red
    cargo clean
    if (Test-Path "$rootDir\app\build") { Remove-Item -Recurse -Force "$rootDir\app\build" }
    if (Test-Path "$rootDir\app\.svelte-kit") { Remove-Item -Recurse -Force "$rootDir\app\.svelte-kit" }
    Write-Host "Full clean completed! Target directory reset." -ForegroundColor Green
} else {
    Write-Host "`n[Mode: Smart Clean] Removing target/debug (saving 90%+ disk space, keeping Release)..." -ForegroundColor Yellow
    if (Test-Path "$rootDir\target\debug") {
        Remove-Item -Recurse -Force "$rootDir\target\debug"
    }
    Write-Host "Smart clean completed! Freed $initialDebugSize MB ($debugGB GB) of disk space!" -ForegroundColor Green
    Write-Host "Note: Release binaries are preserved in target/release." -ForegroundColor DarkGray
}

$finalTotalSize = Get-FolderSizeMB "$rootDir\target"
Write-Host "`nFinal target size: $finalTotalSize MB`n" -ForegroundColor Cyan
