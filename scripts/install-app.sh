#!/bin/sh
# Install the built app into /Applications (quits the running copy first) and open it.
set -e
APP="$(cd "$(dirname "$0")/.." && pwd)/target/release/bundle/macos/openclicky.app"
osascript -e 'quit app "openclicky"' >/dev/null 2>&1 || true
pkill -f "openclicky.app/Contents/MacOS" >/dev/null 2>&1 || true
sleep 1
rm -rf /Applications/openclicky.app
ditto "$APP" /Applications/openclicky.app
open /Applications/openclicky.app
echo "Installed /Applications/openclicky.app"
