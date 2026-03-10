#!/bin/bash

# ============================================================================
# Lumen Uninstallation Script 👋
# "It's not you, it's me."
# ============================================================================

set -e

# OS Detection
OS=$(uname -s)

# Configuration based on OS
if [ "$OS" == "Darwin" ]; then
    INSTALL_DIR="/Applications"
    APP_NAME="Lumen.app"
    CONFIG_DIR="$HOME/Library/Application Support/lumen"
    DB_PATH="$HOME/Library/Application Support/com.sijirama.lumen"
    BINARY_NAME=""
else
    INSTALL_DIR="/usr/local/bin"
    BINARY_NAME="lumen"
    CONFIG_DIR="$HOME/.config/lumen"
    DB_PATH="$HOME/.local/share/com.sijirama.lumen"
    DESKTOP_FILE="$HOME/.local/share/applications/lumen.desktop"
    ICON_FILE="$HOME/.local/share/icons/lumen.png"
fi

# Colors
RED='\033[0;31m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${RED}😢 Moving on already? Lumen is packing bags...${NC}"

# 1. Stop the process
if pgrep -x "lumen" > /dev/null 2>&1; then
    echo -e "${BLUE}🛑 Stopping running instances of Lumen...${NC}"
    pkill -x "lumen" || sudo pkill -x "lumen" 2>/dev/null || true
fi

# 2. Remove Application/Binary
if [ "$OS" == "Darwin" ]; then
    if [ -d "$INSTALL_DIR/$APP_NAME" ]; then
        echo -e "${BLUE}🗑 Removing Lumen app from $INSTALL_DIR...${NC}"
        rm -rf "$INSTALL_DIR/$APP_NAME"
        echo -e "${BLUE}✅ App removed.${NC}"
    fi
else
    if [ -f "$INSTALL_DIR/$BINARY_NAME" ]; then
        echo -e "${BLUE}🗑 Removing binary from $INSTALL_DIR (might need sudo)...${NC}"
        sudo rm "$INSTALL_DIR/$BINARY_NAME"
        echo -e "${BLUE}✅ Binary removed.${NC}"
    fi

    # 2.1 Remove Linux Desktop Integration
    [ -f "$DESKTOP_FILE" ] && rm "$DESKTOP_FILE"
    [ -f "$ICON_FILE" ] && rm "$ICON_FILE"
    rm -f "$HOME/.config/autostart/lumen.desktop"
fi

# 3. Clean up data
echo -e "${YELLOW}Do you want to wipe my memory too? This deletes your history and settings. [y/N]${NC}"
read -r wipe

if [[ "$wipe" =~ ^[Yy]$ ]]; then
    echo -e "${BLUE}🧹 Wiping config and database...${NC}"
    rm -rf "$CONFIG_DIR"
    rm -rf "$DB_PATH"
    echo -e "${BLUE}✅ Memory cleared.${NC}"
else
    echo -e "${BLUE}Keeping your settings safe in case you miss me! 💖${NC}"
fi

echo -e "${RED}Done. Lumen has been uninstalled. stay wavy. ✨${NC}"
