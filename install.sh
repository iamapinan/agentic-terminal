#!/usr/bin/env bash
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$DIR"

echo "=== 1. Building release binary with Cargo ==="
cargo build --release

BIN_NAME="agentic-terminal"
SRC_BIN="$DIR/target/release/rust-agent-tui"
INSTALL_BIN_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_BIN_DIR"
cp "$SRC_BIN" "$INSTALL_BIN_DIR/$BIN_NAME"
chmod +x "$INSTALL_BIN_DIR/$BIN_NAME"

echo "=== 2. Installed CLI binary to $INSTALL_BIN_DIR/$BIN_NAME ==="

echo "=== 3. Creating macOS Application Bundle ==="
APP_DIR="$HOME/Applications/Agentic Terminal.app"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Copy Icon
if [ -f "$DIR/assets/AppIcon.icns" ]; then
    cp "$DIR/assets/AppIcon.icns" "$APP_DIR/Contents/Resources/AppIcon.icns"
fi

cat << 'EOF' > "$APP_DIR/Contents/Info.plist"
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
    <key>LSUIElement</key>
    <false/>
</dict>
</plist>
EOF

cat << EOF > "$APP_DIR/Contents/MacOS/launcher"
#!/usr/bin/env bash
BIN="$INSTALL_BIN_DIR/$BIN_NAME"

if [ -t 0 ]; then
    exec "\$BIN" "\$@"
fi

osascript << 'APPLESCRIPT'
tell application "Terminal"
    activate
    do script "$INSTALL_BIN_DIR/$BIN_NAME"
end tell
APPLESCRIPT
EOF

chmod +x "$APP_DIR/Contents/MacOS/launcher"
touch "$APP_DIR"

echo "=== 4. Installation Complete! ==="
echo "Application installed at: $APP_DIR"
