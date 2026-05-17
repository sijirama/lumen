# ============================================================================
# Lumen Uninstallation Script (Windows)
# Untested — open an issue if it breaks.
# ============================================================================

$BINARY_NAME = "lumen"
$APP_NAME = "Lumen"
$IDENT = "com.sijirama.lumen"

Write-Host "`nUninstalling Lumen..." -ForegroundColor Blue

# 1. Stop running process
$proc = Get-Process -Name $BINARY_NAME -ErrorAction SilentlyContinue
if ($proc) {
    Write-Host "Stopping running Lumen process..." -ForegroundColor Blue
    Stop-Process -Name $BINARY_NAME -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
}

# 2. Uninstall the MSI package
Write-Host "Removing installed package..." -ForegroundColor Blue
$pkg = Get-Package -Name $APP_NAME -ErrorAction SilentlyContinue
if ($pkg) {
    $pkg | Uninstall-Package -Force | Out-Null
    Write-Host "Package removed." -ForegroundColor Green
} else {
    # Fallback: WMI lookup (older method, slower but reliable)
    $wmi = Get-WmiObject -Class Win32_Product -Filter "Name like 'Lumen%'" -ErrorAction SilentlyContinue
    if ($wmi) {
        $wmi.Uninstall() | Out-Null
        Write-Host "Package removed (via WMI)." -ForegroundColor Green
    } else {
        Write-Host "No installed Lumen package found." -ForegroundColor Yellow
    }
}

# 3. Remove autostart registry entry (Tauri autostart plugin writes HKCU Run)
$runKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$autostartName = $IDENT
if (Get-ItemProperty -Path $runKey -Name $autostartName -ErrorAction SilentlyContinue) {
    Remove-ItemProperty -Path $runKey -Name $autostartName -Force
    Write-Host "Autostart entry removed." -ForegroundColor Blue
}

# 4. Optionally wipe data
$wipe = Read-Host "Wipe Lumen's data too? This deletes your history, memories, API keys. [y/N]"
if ($wipe -match "^[Yy]$") {
    $paths = @(
        "$env:APPDATA\$IDENT",
        "$env:LOCALAPPDATA\$IDENT",
        "$env:APPDATA\$BINARY_NAME",
        "$env:LOCALAPPDATA\$BINARY_NAME"
    )
    foreach ($path in $paths) {
        if (Test-Path $path) {
            Remove-Item -Path $path -Recurse -Force -ErrorAction SilentlyContinue
            Write-Host "Removed $path" -ForegroundColor Blue
        }
    }
    Write-Host "Data wiped." -ForegroundColor Green
} else {
    Write-Host "Settings kept." -ForegroundColor Blue
}

Write-Host "`nDone. Lumen uninstalled.`n" -ForegroundColor Red
