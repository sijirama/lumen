# ============================================================================
# Lumen Installation Script ✨ (Windows)
# "Easy peasy lemon squeezy."
# ============================================================================

$REPO = "sijirama/lumen"
$BINARY_NAME = "lumen"

Write-Host "`n   __                                " -ForegroundColor Blue
Write-Host "  / /_  __ ____ ___  ___  ____       " -ForegroundColor Blue
Write-Host " / / / / / __ \`__ \/ _ \/ __ \      " -ForegroundColor Blue
Write-Host "/ / /_/ / / / / / /  __/ / / /      " -ForegroundColor Blue
Write-Host "/_/\__,_/_/ /_/ /_/\___/_/ /_/  ✨  `n" -ForegroundColor Blue

Write-Host "Starting the Lumen setup... No cap, this will be quick." -ForegroundColor Blue

# 1. Fetch Latest Release
Write-Host "🚀 Fetching the latest sizzle from GitHub..." -ForegroundColor Blue
$releases = Invoke-RestMethod -Uri "https://api.github.com/repos/$REPO/releases/latest"
$LATEST_RELEASE = $releases.tag_name

if (-not $LATEST_RELEASE) {
    Write-Host "Couldn't find a release tagged on GitHub. Fallback to v0.1.0" -ForegroundColor Yellow
    $LATEST_RELEASE = "v0.1.0"
}

# 2. Download and Install
# Asset naming convention for Tauri on Windows: Lumen_0.1.3_x64_en-US.msi
$VERSION_NUM = $LATEST_RELEASE.Replace("v", "")
$ASSET_NAME = "Lumen_${VERSION_NUM}_x64_en-US.msi"
$DOWNLOAD_URL = "https://github.com/$REPO/releases/download/$LATEST_RELEASE/$ASSET_NAME"

$TEMP_FILE = "$env:TEMP\lumen_setup.msi"

Write-Host "📦 Downloading Lumen $LATEST_RELEASE ($ASSET_NAME)..." -ForegroundColor Blue
try {
    Invoke-WebRequest -Uri $DOWNLOAD_URL -OutFile $TEMP_FILE
} catch {
    Write-Host "❌ Failed to download latest release. Please check your internet connection or the repository." -ForegroundColor Red
    exit 1
}

# 3. Run MSI Installer
Write-Host "🔧 Launching the installer..." -ForegroundColor Blue
Start-Process -FilePath "msiexec.exe" -ArgumentList "/i `"$TEMP_FILE`"" -Wait

Write-Host "`n🎉 Lumen installation process complete! Check your Start menu." -ForegroundColor Green
Write-Host "Stay wavy. ✨`n" -ForegroundColor Blue
