#!/bin/bash

# ============================================================================
# Lumen Uninstallation Script 👋
# "It's not you, it's me."
# ============================================================================

set -e

# Configuration
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="lumen"
CONFIG_DIR="$HOME/.config/lumen"
DB_PATH="$HOME/.local/share/com.sijirama.lumen"

# Colors
RED='\033[0;31m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${RED}😢 Moving on already? Lumen is packing bags...${NC}"

# 1. Stop the process
if pgrep -x "lumen" > /dev/null; then
    echo -e "${BLUE}🛑 Stopping running instances of Lumen... No cap, just closing shop.${NC}"
    pkill -x "lumen" || sudo pkill -x "lumen"
fi

# 2. Remove Binary
if [ -f "$INSTALL_DIR/$BINARY_NAME" ]; then
    echo -e "${BLUE}🗑 Removing binary from $INSTALL_DIR (might need sudo)...${NC}"
    sudo rm "$INSTALL_DIR/$BINARY_NAME"
    echo -e "${BLUE}✅ Binary removed.${NC}"
else
    echo -e "${YELLOW}Binary wasn't found in $INSTALL_DIR. Maybe it's playing hide and seek?${NC}"
fi

# 3. Clean up data (Optional but recommended for full wipe)
echo -e "${YELLOW}Do you want to wipe my memory too? This deletes your history and settings. [y/N]${NC}"
read -r wipe

if [[ "$wipe" =~ ^[Yy]$ ]]; then
    echo -e "${BLUE}🧹 Wiping config and database... Pure slate mode initialized.${NC}"
    rm -rf "$CONFIG_DIR"
    rm -rf "$DB_PATH"
    echo -e "${BLUE}✅ Memory cleared.${NC}"
else
    echo -e "${BLUE}Keeping your settings safe in case you miss me! 💖${NC}"
fi

echo -e "${RED}Done. Lumen has been uninstalled. What else we cookin' up? (Wait, I'm gone now...)${NC}"
echo -e "${BLUE}Hope to see you again soon! ✨${NC}"
