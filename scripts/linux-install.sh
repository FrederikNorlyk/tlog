#!/bin/bash

set -e

# Colors for output
GREEN='\033[0;32m'
NC='\033[0m' # No Color

echo "Creating temporary directory"
TEMP_DIR=$(mktemp -d)
trap 'rm -rf $TEMP_DIR' EXIT

echo "Downloading..."
curl -sSL https://github.com/FrederikNorlyk/tlog/releases/latest/download/tlog-linux-x86_64.tar.gz -o "$TEMP_DIR/tlog.tar.gz"

echo "Extracting..."
tar -xzf "$TEMP_DIR/tlog.tar.gz" -C "$TEMP_DIR"

echo "Installing..."
sudo install -m 755 "$TEMP_DIR/tlog" /usr/local/bin/tlog

echo -e "${GREEN}Installation complete!${NC}"