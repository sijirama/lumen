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

# 5. Desktop Integration
echo -e "${BLUE}🖥 Integrating with your desktop launcher...${NC}"
mkdir -p "$ICON_DIR"
mkdir -p "$DESKTOP_DIR"

# Download or Copy icon
if [ -f "./public/logo.png" ]; then
    echo "Found local logo.png, using that."
    cp "./public/logo.png" "$ICON_DIR/lumen.png"
elif [ -f "./src-tauri/icons/128x128.png" ]; then
    echo "Found local tauri icons, using that."
    cp "./src-tauri/icons/128x128.png" "$ICON_DIR/lumen.png"
else
    echo "Downloading icon from GitHub..."
    curl -s -L -o "$ICON_DIR/lumen.png" "$ICON_URL"
fi

# Create Desktop Entry
cat <<EOF > "$DESKTOP_DIR/lumen.desktop"
[Desktop Entry]
Name=Lumen
Comment=Your AI-powered desktop sidekick
Exec=$BINARY_NAME
Icon=$ICON_DIR/lumen.png
Terminal=false
Type=Application
Categories=Utility;Contextual;AI;
Keywords=ai;chat;lumen;assistant;
EOF

chmod +x "$DESKTOP_DIR/lumen.desktop"
echo -e "${GREEN}✅ Desktop entry created. You can now find Lumen in your applications menu! ✨${NC}"

echo -e "${BLUE}What else we cookin' up today? Let's check some vibes...${NC}"

# 5. Run it immediately
echo -e "${GREEN}🚀 Launching Lumen now! ✨${NC}"
lumen &

echo -e "${BLUE}Lumen is now running in the background. Tap your global shortcut or find the icon in your tray. ✨${NC}"
