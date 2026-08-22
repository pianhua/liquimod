<#
.SYNOPSIS
Removes known, regenerable LiquiMod build directories.

.EXAMPLE
.\scripts\clean.ps1 -DryRun
#>
[CmdletBinding()]
param(
    [switch]$DebugOnly,
    [switch]$All,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

if ($DebugOnly -and $All) {
    throw "-DebugOnly and -All cannot be used together."
}

$rootDir = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
Set-Location $rootDir

function Get-FolderSizeMB([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return 0 }
    $sum = (Get-ChildItem -LiteralPath $Path -Recurse -File -Force -ErrorAction SilentlyContinue |
        Measure-Object -Property Length -Sum).Sum
    if ($null -eq $sum) { return 0 }
    return [math]::Round($sum / 1MB, 2)
}

function Remove-WorkspaceDirectory([string]$Path) {
    $fullPath = [System.IO.Path]::GetFullPath($Path).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
    $rootPrefix = $rootDir.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $fullPath.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to clean a path outside the workspace: $fullPath"
    }
    if (-not (Test-Path -LiteralPath $fullPath)) {
        Write-Host "Already absent: $fullPath" -ForegroundColor DarkGray
        return
    }
    if (-not (Get-Item -LiteralPath $fullPath).PSIsContainer) {
        throw "Refusing to clean a non-directory path: $fullPath"
    }
    if ($DryRun) {
        Write-Host "Would remove: $fullPath" -ForegroundColor Yellow
        return
    }
    [System.IO.Directory]::Delete($fullPath, $true)
    Write-Host "Removed: $fullPath" -ForegroundColor Green
}

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "  LiquiMod Workspace Disk Cleanup Tool" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan

$targetRoot = Join-Path $rootDir "target"
$initialDebugSize = Get-FolderSizeMB (Join-Path $targetRoot "debug")
$initialReleaseSize = Get-FolderSizeMB (Join-Path $targetRoot "release")
$initialTotalSize = Get-FolderSizeMB $targetRoot

Write-Host "`nCurrent target size:" -ForegroundColor Yellow
Write-Host "  * target/debug   (test & debug cache): $initialDebugSize MB"
Write-Host "  * target/release (production build)  : $initialReleaseSize MB"
Write-Host "  * target total                       : $initialTotalSize MB"

if ($All) {
    Write-Host "`n[Mode: Full Clean] Removing only known build directories..." -ForegroundColor Yellow
    $paths = @(
        (Join-Path $targetRoot "debug"),
        (Join-Path $targetRoot "release"),
        (Join-Path $rootDir "app\build"),
        (Join-Path $rootDir "app\.svelte-kit")
    )
} else {
    Write-Host "`n[Mode: Smart Clean] Removing debug caches and keeping release binaries..." -ForegroundColor Yellow
    $paths = @((Join-Path $targetRoot "debug"))
}

if ($DebugOnly) {
    Write-Host "`n[Mode: Debug Only] Removing target/debug only..." -ForegroundColor Yellow
    $paths = @((Join-Path $targetRoot "debug"))
}

foreach ($path in $paths) {
    Remove-WorkspaceDirectory $path
}

$finalTotalSize = Get-FolderSizeMB $targetRoot
Write-Host "`nFinal target size: $finalTotalSize MB" -ForegroundColor Cyan
if ($DryRun) {
    Write-Host "Dry run complete; no directories were removed." -ForegroundColor Yellow
}
