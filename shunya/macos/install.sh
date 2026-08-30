#!/bin/bash
set -e

echo "Installing Shunya macOS Daemon..."

if [ "$EUID" -ne 0 ]; then
  echo "Please run as root (sudo ./install.sh)"
  exit 1
fi

DAEMON_BIN="/usr/local/sbin/shunyad"
ENGINE_BIN="/usr/local/sbin/shunya-engine"
PLIST_FILE="local.shunya.shunyad.plist"
PLIST_DEST="/Library/LaunchDaemons/$PLIST_FILE"

# 1. Install binaries
echo "Installing binaries..."
mkdir -p /usr/local/sbin
cp ../go/bin/shunyad "$DAEMON_BIN"
cp ../crates/target/release/shunya-engine "$ENGINE_BIN"
chmod 755 "$DAEMON_BIN"
chmod 755 "$ENGINE_BIN"
chown root:wheel "$DAEMON_BIN"
chown root:wheel "$ENGINE_BIN"

# 2. Install plist
echo "Installing launchd plist..."
cp "$PLIST_FILE" "$PLIST_DEST"
chmod 644 "$PLIST_DEST"
chown root:wheel "$PLIST_DEST"

# 3. Load the daemon
echo "Loading daemon via launchctl..."
# Unload it first just in case it's already running
launchctl unload "$PLIST_DEST" 2>/dev/null || true
launchctl load -w "$PLIST_DEST"

echo "Installation complete. Verify with: tail -f /var/log/shunyad.log"
