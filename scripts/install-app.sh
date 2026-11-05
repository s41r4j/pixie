#!/bin/sh
# Install the built app into /Applications (quits any running copy, old name included) and open it.
set -e
APP="$(cd "$(dirname "$0")/.." && pwd)/target/release/bundle/macos/Pixie.app"
osascript -e 'quit app "Pixie"' >/dev/null 2>&1 || true
osascript -e 'quit app "openclicky"' >/dev/null 2>&1 || true
pkill -f "Pixie.app/Contents/MacOS" >/dev/null 2>&1 || true
pkill -f "openclicky.app/Contents/MacOS" >/dev/null 2>&1 || true
sleep 1
# The old name is replaced by Pixie (same internal identity, so permissions and data carry over).
rm -rf /Applications/openclicky.app /Applications/Pixie.app
ditto "$APP" /Applications/Pixie.app
open /Applications/Pixie.app
echo "Installed /Applications/Pixie.app"
