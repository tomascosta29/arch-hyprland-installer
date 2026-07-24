#!/bin/bash

INSTALL_DIR="$HOME/.local/share/costa-utils"
BIN_DIR="$HOME/.local/bin"
APP_DIR="$HOME/.local/share/applications"

echo "Uninstalling CostaUtils..."

# Remove desktop entries
echo "Removing desktop entries..."
rm -f "$APP_DIR/org.fcosta.CostaUtils.AppMenu.desktop"
rm -f "$APP_DIR/org.fcosta.CostaUtils.Runner.desktop"
rm -f "$APP_DIR/org.fcosta.CostaUtils.Blinker.desktop"
rm -f "$APP_DIR/org.fcosta.CostaUtils.BlinkerManager.desktop"
rm -f "$APP_DIR/org.fcosta.CostaUtils.Clipper.desktop"
rm -f "$APP_DIR/org.fcosta.CostaUtils.Power.desktop"
rm -f "$APP_DIR/org.fcosta.CostaUtils.Network.desktop"
rm -f "$APP_DIR/org.fcosta.CostaUtils.Bluetooth.desktop"
rm -f "$APP_DIR/org.fcosta.CostaUtils.Volume.desktop"
rm -f "$APP_DIR/org.fcosta.CostaUtils.ControlCenter.desktop"

# Remove symlink
echo "Removing symlink..."
rm -f "$BIN_DIR/costa-utils"
rm -f "$BIN_DIR/app-menu"
rm -f "$BIN_DIR/runner"
rm -f "$BIN_DIR/blinker"
rm -f "$BIN_DIR/blinker-manager"
rm -f "$BIN_DIR/clipper"
rm -f "$BIN_DIR/power-menu"
rm -f "$BIN_DIR/network-menu"
rm -f "$BIN_DIR/bluetooth-menu"
rm -f "$BIN_DIR/volume-menu"
rm -f "$BIN_DIR/control-center"

# Remove installation directory
if [ -d "$INSTALL_DIR" ]; then
    echo "Removing installation directory ($INSTALL_DIR)..."
    rm -rf "$INSTALL_DIR"
fi

echo "Uninstallation complete!"
