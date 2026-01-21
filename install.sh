#!/bin/bash

# ============================================================================
# Lumen Installation Script ✨
# "Easy peasy lemon squeezy."
# ============================================================================

set -e

# Configuration
REPO="sijirama/lumen"
BINARY_NAME="lumen"
INSTALL_DIR="/usr/local/bin"

# Colors for that premium vibe
BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Desktop Integration Paths
ICON_DIR="$HOME/.local/share/icons"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_URL="https://raw.githubusercontent.com/$REPO/main/src-tauri/icons/128x128.png"

echo -e "${BLUE}"
echo "   __                                "
echo "  / /_  __ ____ ___  ___  ____       "
echo " / / / / / __ \`__ \/ _ \/ __ \      "
echo "/ / /_/ / / / / / /  __/ / / /      "
echo "/_/\__,_/_/ /_/ /_/\___/_/ /_/  ✨  "
echo -e "${NC}"

echo -e "${BLUE}Starting the Lumen setup... No cap, this will be quick.${NC}"

# ============================================================================
# Check for existing installation (Update Mode)
# ============================================================================
EXISTING_BINARY="$INSTALL_DIR/$BINARY_NAME"

if [ -f "$EXISTING_BINARY" ]; then
    echo -e "${YELLOW}🔄 Existing Lumen installation detected at $EXISTING_BINARY${NC}"
    
    # Check if Lumen is currently running and kill it
    if pgrep -x "$BINARY_NAME" > /dev/null 2>&1; then
        echo -e "${YELLOW}⚠️  Lumen is currently running. Stopping it...${NC}"
        pkill -x "$BINARY_NAME" 2>/dev/null || true
        # Give it a moment to fully terminate
        sleep 1
        # Force kill if still running
        if pgrep -x "$BINARY_NAME" > /dev/null 2>&1; then
            echo -e "${YELLOW}   Force killing stubborn process...${NC}"
            pkill -9 -x "$BINARY_NAME" 2>/dev/null || true
            sleep 1
        fi
        echo -e "${GREEN}   ✓ Lumen process stopped.${NC}"
    fi
    
    # Remove the old binary
    echo -e "${YELLOW}🗑  Removing old Lumen binary...${NC}"
    sudo rm -f "$EXISTING_BINARY"
    echo -e "${GREEN}   ✓ Old installation removed. Ready for fresh install!${NC}"
    echo ""
else
    echo -e "${BLUE}📦 Fresh installation detected. Let's get you set up!${NC}"
fi

# 1. OS & Arch Detection
OS=$(uname -s)
ARCH=$(uname -m)

if [ "$OS" != "Linux" ]; then
    echo -e "${RED}Hold up! This script is currently optimized for Linux.${NC}"
    echo -e "${YELLOW}Please visit https://github.com/$REPO/releases for other platforms.${NC}"
    exit 1
fi

# 2. Dependency Check (Essential for Tauri apps on Linux)
echo -e "${BLUE}🏗 Checking for survivors (dependencies)...${NC}"

# If the header exists, we skip the apt install to avoid conflicts
if [ -d "/usr/include/webkitgtk-4.1" ] || [ -d "/usr/include/webkitgtk-4.0" ]; then
    echo -e "${GREEN}Developer headers detected. Skipping system library install. ✨${NC}"
else
    if command -v apt-get >/dev/null; then
        echo "Installing missing system libraries..."
        sudo apt-get update -qq
        sudo apt-get install -qq -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev
    else
        echo -e "${YELLOW}Non-Debian system detected. Please ensure webkit2gtk-4.1 is installed!${NC}"
    fi
fi

# 3. Fetch Latest Release
echo -e "${BLUE}🚀 Fetching the latest sizzle from GitHub...${NC}"
LATEST_RELEASE=$(curl -s https://api.github.com/repos/$REPO/releases/latest | grep "tag_name" | cut -d '"' -f 4)

if [ -z "$LATEST_RELEASE" ]; then
    # Fallback if API fails or no release exists yet
    echo -e "${YELLOW}Couldn't find a release tagged on GitHub. Checking binary in local build...${NC}"
    LATEST_RELEASE="v0.1.0"
fi

# 4. Download and Install
# Asset naming convention for Tauri: Lumen_0.1.0_amd64.AppImage
ARCH_SUFFIX="amd64"
if [ "$ARCH" == "aarch64" ]; then ARCH_SUFFIX="arm64"; fi

VERSION_NUM=$(echo $LATEST_RELEASE | sed 's/v//')
ASSET_NAME="Lumen_${VERSION_NUM}_${ARCH_SUFFIX}.AppImage"
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$LATEST_RELEASE/$ASSET_NAME"

echo -e "${BLUE}📦 Downloading Lumen $LATEST_RELEASE ($ASSET_NAME)...${NC}"

# Check if online asset exists, else fallback to local check
if curl --output /dev/null --silent --head --fail "$DOWNLOAD_URL"; then
    curl -L -o /tmp/lumen_temp "$DOWNLOAD_URL"
else
    echo -e "${YELLOW}Release asset not found online yet. Checking local builds...${NC}"
    # Look for both raw binary and AppImage in target/release
    LOCAL_BINARY="./src-tauri/target/release/lumen"
    LOCAL_APPIMAGE=$(find ./src-tauri/target/release/bundle/appimage -name "*.AppImage" | head -n 1)
    
    if [ -f "$LOCAL_BINARY" ]; then
        echo -e "${GREEN}Found a local binary! Using that.${NC}"
        cp "$LOCAL_BINARY" /tmp/lumen_temp
    elif [ -n "$LOCAL_APPIMAGE" ]; then
        echo -e "${GREEN}Found a local AppImage! Using that.${NC}"
        cp "$LOCAL_APPIMAGE" /tmp/lumen_temp
    else
        echo -e "${RED}Installation failed: No binary or AppImage found. Run 'npm run tauri build' first!${NC}"
        exit 1
    fi
fi

# Move to bin and make executable
echo -e "${BLUE}🔧 Installing Lumen to $INSTALL_DIR... (sudo required)${NC}"
sudo mv /tmp/lumen_temp "$INSTALL_DIR/$BINARY_NAME"
sudo chmod +x "$INSTALL_DIR/$BINARY_NAME"

echo -e "${GREEN}🎉 Lumen is successfully installed!${NC}"

# 5. Desktop Integration & Autostart
echo -e "${BLUE}🖥 Integrating with your desktop launcher & autostart...${NC}"

AUTOSTART_DIR="$HOME/.config/autostart"
mkdir -p "$ICON_DIR"
mkdir -p "$DESKTOP_DIR"
mkdir -p "$AUTOSTART_DIR"

# Cleanup any previous broken entries (Case-sensitive cleanup)
rm -f "$DESKTOP_DIR/Lumen.desktop" "$DESKTOP_DIR/lumen.desktop"
rm -f "$AUTOSTART_DIR/Lumen.desktop" "$AUTOSTART_DIR/lumen.desktop"

# Download or Copy icon
if [ -f "./public/logo.png" ]; then
    cp "./public/logo.png" "$ICON_DIR/lumen.png"
elif [ -f "./src-tauri/icons/128x128.png" ]; then
    cp "./src-tauri/icons/128x128.png" "$ICON_DIR/lumen.png"
else
    curl -s -L -o "$ICON_DIR/lumen.png" "$ICON_URL"
fi

# Create high-quality Desktop Entry with absolute path
cat <<EOF > "$DESKTOP_DIR/lumen.desktop"
[Desktop Entry]
Name=Lumen
Comment=Your AI-powered desktop sidekick
Exec=$INSTALL_DIR/$BINARY_NAME
Icon=$ICON_DIR/lumen.png
Terminal=false
Type=Application
Categories=Utility;Contextual;AI;
Keywords=ai;chat;lumen;assistant;
StartupNotify=false
EOF

chmod +x "$DESKTOP_DIR/lumen.desktop"

# Copy to Autostart so it boots with the OS (with --minimized flag)
cat <<EOF > "$AUTOSTART_DIR/lumen.desktop"
[Desktop Entry]
Name=Lumen
Comment=Your AI-powered desktop sidekick
Exec=$INSTALL_DIR/$BINARY_NAME --minimized
Icon=$ICON_DIR/lumen.png
Terminal=false
Type=Application
Categories=Utility;Contextual;AI;
Keywords=ai;chat;lumen;assistant;
StartupNotify=false
EOF
chmod +x "$AUTOSTART_DIR/lumen.desktop"

# Refresh desktop database so icon appears immediately (if tool exists)
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$DESKTOP_DIR" >/dev/null 2>&1 || true
fi

echo -e "${GREEN}✅ Desktop & Autostart integration complete. ✨${NC}"

# 6. Run it immediately (Detached from terminal)
echo -e "${GREEN}🚀 Launching Lumen (Background Mode)...${NC}"
# Use nohup and redirect output to ensure it survives terminal closing
nohup "$INSTALL_DIR/$BINARY_NAME" > /dev/null 2>&1 &

echo -e "${BLUE}Lumen is now running. Close this terminal and stay wavy. ✨${NC}"
