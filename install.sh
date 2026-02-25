#!/bin/bash

# Ensure running as root
if [ "$EUID" -ne 0 ]; then
  echo "Please run as root (sudo ./install.sh)"
  exit 1
fi

SERVICE_FILE="linux-nitrosense.service"
DESKTOP_FILE="linux-nitrosense.desktop"
BINARY_PATH="target/release/linux-nitrosense"
BUNDLED_BIN="./linux-nitrosense"
INSTALL_BIN="/usr/local/bin/linux-nitrosense"
REPO="Packss/linux-nitrosense-rust"

# 1. Check for bundled binary in current directory
if [ -f "$BUNDLED_BIN" ]; then
    echo "Found bundled binary at $BUNDLED_BIN. Installing..."
    cp "$BUNDLED_BIN" "$INSTALL_BIN"

# 2. Check for local build artifact
elif [ -f "$BINARY_PATH" ]; then
    echo "Found local build at $BINARY_PATH. Installing..."
    cp "$BINARY_PATH" "$INSTALL_BIN"

# 3. Ask user: Download or Build?
else
    echo "Binary not found locally."
    read -p "Do you want to [d]ownload the latest release or [b]uild from source? (d/b): " choice
    
    if [ "$choice" = "d" ] || [ "$choice" = "D" ]; then
        echo "Downloading latest release from GitHub ($REPO)..."
        # Fetch latest release tag
        LATEST_URL=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep "browser_download_url.*linux-nitrosense" | cut -d '"' -f 4)
        
        if [ -z "$LATEST_URL" ]; then
            echo "Error: Could not find a release asset named 'linux-nitrosense' in $REPO."
            echo "Falling back to build from source..."
            choice="b"
        else
            wget -O "$INSTALL_BIN" "$LATEST_URL" || curl -L -o "$INSTALL_BIN" "$LATEST_URL"
            if [ $? -ne 0 ]; then
                echo "Download failed."
                exit 1
            fi
            echo "Download complete."
        fi
    fi

    if [ "$choice" = "b" ] || [ "$choice" = "B" ]; then
        echo "Building from source..."
        if command -v cargo &> /dev/null; then
             # Build as non-root user if SUDO_USER is set
            if [ -n "$SUDO_USER" ]; then
                sudo -u "$SUDO_USER" cargo build --release
            else
                cargo build --release
            fi
            
            if [ ! -f "$BINARY_PATH" ]; then
                echo "Build failed."
                exit 1
            fi
            cp "$BINARY_PATH" "$INSTALL_BIN"
        else
             echo "Cargo not found. Please install Rust or choose download option."
             exit 1
        fi
    fi
fi

if [ ! -f "$INSTALL_BIN" ]; then
    echo "Installation failed: Binary not created."
    exit 1
fi

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
