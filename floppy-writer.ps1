# ============================================================================
# write-floppy.ps1 - Write Memory Map Tool to Floppy/USB
# ============================================================================
#
# Usage (Run as Administrator!):
#   .\write-floppy.ps1                    # Writes to floppy drive A:
#   .\write-floppy.ps1 -USB 1             # Writes to PhysicalDrive1
#   .\write-floppy.ps1 -ListDisks         # List available disks
#   .\write-floppy.ps1 -Image .\my.img    # Use custom image file
#
# ============================================================================

param(
    [string]$Image = "",
    [int]$USB = -1,
    [switch]$Floppy,
    [switch]$ListDisks,
    [switch]$Force
)

# Find the image file
function Find-Image {
    $searchPaths = @(
        "output\gameboy-system.img"
    )

    foreach ($path in $searchPaths) {
        if (Test-Path $path) {
            return (Resolve-Path $path).Path
        }
    }
    return $null
}

# List disks
if ($ListDisks) {
    Write-Host "`nAvailable Disks:" -ForegroundColor Cyan
    Get-Disk | Format-Table Number, FriendlyName, @{L='Size (MB)';E={[math]::Round($_.Size/1MB)}}, PartitionStyle, OperationalStatus
    Write-Host "Use: .\write-floppy.ps1 -USB <Number>" -ForegroundColor Yellow
    exit 0
}

# Resolve image path
if ($Image -eq "") {
    $Image = Find-Image
    if ($null -eq $Image) {
        Write-Host "ERROR: Could not find gameboy-system.img" -ForegroundColor Red
        Write-Host "Searched: gameboy-system.img, build\gameboy-system.img, output\gameboy-system.img" -ForegroundColor Yellow
        Write-Host "`nBuild first with: docker build -t gb-os-builder . && docker run --rm -v `${PWD}/output:/output gb-os-builder" -ForegroundColor Cyan
        exit 1
    }
}

if (-not (Test-Path $Image)) {
    Write-Host "ERROR: Image file not found: $Image" -ForegroundColor Red
    exit 1
}

$imgSize = (Get-Item $Image).Length
Write-Host "`nImage: $Image ($imgSize bytes)" -ForegroundColor Green

# Determine target
if ($USB -ge 0) {
    $target = "\\.\PhysicalDrive$USB"
    $diskInfo = Get-Disk -Number $USB -ErrorAction SilentlyContinue
    if ($null -eq $diskInfo) {
        Write-Host "ERROR: PhysicalDrive$USB not found" -ForegroundColor Red
        Write-Host "Use: .\write-floppy.ps1 -ListDisks" -ForegroundColor Yellow
        exit 1
    }
    Write-Host "Target: $target ($($diskInfo.FriendlyName))" -ForegroundColor Yellow
} elseif ($Floppy -or $USB -eq -1) {
    $target = "\\.\A:"
    Write-Host "Target: $target (Floppy Drive)" -ForegroundColor Yellow
} else {
    Write-Host "ERROR: Specify -Floppy or -USB <number>" -ForegroundColor Red
    exit 1
}

# Confirm
if (-not $Force) {
    Write-Host "`nWARNING: This will OVERWRITE all data on $target!" -ForegroundColor Red
    $confirm = Read-Host "Type YES to continue"
    if ($confirm -ne "YES") {
        Write-Host "Aborted." -ForegroundColor Yellow
        exit 0
    }
}

# Write image
try {
    Write-Host "`nWriting image..." -ForegroundColor Cyan

    $imgBytes = [System.IO.File]::ReadAllBytes($Image)
    $stream = [System.IO.FileStream]::new($target, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
    $stream.Write($imgBytes, 0, $imgBytes.Length)
    $stream.Flush()
    $stream.Close()

    Write-Host "`nSUCCESS: Wrote $imgSize bytes to $target" -ForegroundColor Green
    Write-Host "You can now boot from this disk!" -ForegroundColor Cyan

} catch {
    Write-Host "`nERROR: Failed to write to $target" -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Red
    Write-Host "`nMake sure you:" -ForegroundColor Yellow
    Write-Host "  1. Run PowerShell as Administrator" -ForegroundColor Yellow
    Write-Host "  2. The disk is not in use" -ForegroundColor Yellow
    Write-Host "  3. The disk number is correct (use -ListDisks)" -ForegroundColor Yellow
    exit 1
}
