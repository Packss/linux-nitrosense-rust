#!/bin/bash

# Ensure running as root
if [ "$EUID" -ne 0 ]; then
  echo "Please run as root (sudo ./install.sh)"
  exit 1
fi

SERVICE_FILE="linux-nitrosense.service"
DESKTOP_FILE="linux-nitrosense.desktop"
BINARY_PATH="target/release/linux-nitrosense"
INSTALL_BIN="/usr/local/bin/linux-nitrosense"

# Check if binary exists, if not try to build (warn if cargo not found)
if [ ! -f "$BINARY_PATH" ]; then
    echo "Binary not found at $BINARY_PATH. Attempting build..."
    if command -v cargo &> /dev/null; then
        # Try to build as the original user if possible to avoid root owned target artifacts
        if [ -n "$SUDO_USER" ]; then
            sudo -u "$SUDO_USER" cargo build --release
        else
            cargo build --release
        fi
        
        if [ ! -f "$BINARY_PATH" ]; then
            echo "Build failed. Please build manually with 'cargo build --release' first."
            exit 1
        fi
    else
        echo "Cargo not found. Please build the project manually first."
        exit 1
    fi
fi

echo "Installing binary to $INSTALL_BIN..."
cp "$BINARY_PATH" "$INSTALL_BIN"
chmod +x "$INSTALL_BIN"

echo "Installing systemd service..."
cp "$SERVICE_FILE" /etc/systemd/system/
systemctl daemon-reload
systemctl enable linux-nitrosense.service
systemctl restart linux-nitrosense.service

echo "Installing desktop entry..."
cp "$DESKTOP_FILE" /usr/share/applications/

echo "Installation complete!"
echo "Service status:"
systemctl status linux-nitrosense.service --no-pager
