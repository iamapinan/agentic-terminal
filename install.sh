#!/usr/bin/env bash
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$DIR"

echo "=== 1. Building release binaries with Cargo (TUI & GUI) ==="
cargo build --release

INSTALL_BIN_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_BIN_DIR"

# Install CLI binaries
cp "$DIR/target/release/agentic-terminal" "$INSTALL_BIN_DIR/agentic-terminal"
cp "$DIR/target/release/agentic-gui" "$INSTALL_BIN_DIR/agentic-gui"
chmod +x "$INSTALL_BIN_DIR/agentic-terminal" "$INSTALL_BIN_DIR/agentic-gui"

echo "=== 2. Installed CLI binaries to $INSTALL_BIN_DIR ==="

# Check PATH
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo "Note: Make sure $HOME/.local/bin is in your PATH (e.g. in ~/.zshrc)"
fi

echo "=== 3. Creating Native macOS Application Bundles ==="

# 3A. Native GUI App: "Agentic GUI.app"
GUI_APP="$HOME/Applications/Agentic GUI.app"
mkdir -p "$GUI_APP/Contents/MacOS"
mkdir -p "$GUI_APP/Contents/Resources"

cp "$DIR/target/release/agentic-gui" "$GUI_APP/Contents/MacOS/agentic-gui"
if [ -f "$DIR/assets/AppIcon.icns" ]; then
    cp "$DIR/assets/AppIcon.icns" "$GUI_APP/Contents/Resources/AppIcon.icns"
fi

cat << 'EOF' > "$GUI_APP/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>agentic-gui</string>
    <key>CFBundleIdentifier</key>
    <string>com.agentic.gui</string>
    <key>CFBundleName</key>
    <string>Agentic GUI</string>
    <key>CFBundleDisplayName</key>
    <string>Agentic GUI</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.2.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.13</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF
chmod +x "$GUI_APP/Contents/MacOS/agentic-gui"
touch "$GUI_APP"

# 3B. Terminal TUI App: "Agentic Terminal.app"
TUI_APP="$HOME/Applications/Agentic Terminal.app"
mkdir -p "$TUI_APP/Contents/MacOS"
mkdir -p "$TUI_APP/Contents/Resources"

if [ -f "$DIR/assets/AppIcon.icns" ]; then
    cp "$DIR/assets/AppIcon.icns" "$TUI_APP/Contents/Resources/AppIcon.icns"
fi

cat << 'EOF' > "$TUI_APP/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>launcher</string>
    <key>CFBundleIdentifier</key>
    <string>com.agentic.terminal</string>
    <key>CFBundleName</key>
    <string>Agentic Terminal</string>
    <key>CFBundleDisplayName</key>
    <string>Agentic Terminal</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.2.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.13</string>
</dict>
</plist>
EOF

cat << EOF > "$TUI_APP/Contents/MacOS/launcher"
#!/usr/bin/env bash
BIN="$INSTALL_BIN_DIR/agentic-terminal"

if [ -t 0 ]; then
    exec "\$BIN" "\$@"
fi

osascript << 'APPLESCRIPT'
tell application "Terminal"
    activate
    do script "$INSTALL_BIN_DIR/agentic-terminal"
end tell
APPLESCRIPT
EOF
chmod +x "$TUI_APP/Contents/MacOS/launcher"
touch "$TUI_APP"

echo "=== 4. Installation Complete! ==="
echo "Installed:"
echo " 1. Native GUI App: $GUI_APP"
echo " 2. Terminal TUI App: $TUI_APP"
echo " 3. CLI Commands: agentic-terminal, agentic-gui"
