#!/bin/sh
# Build, sign and install TouchBar.app, then start it as a LaunchAgent.
set -e
cd "$(dirname "$0")"
cargo build --release
APP="$HOME/Applications/TouchBar.app"
mkdir -p "$APP/Contents/MacOS"
cp bundle/Info.plist "$APP/Contents/Info.plist"
cp target/release/touchbar "$APP/Contents/MacOS/touchbar"
# Plain ad-hoc signature with a STABLE identifier, so an Accessibility grant keyed to
# `identifier "local.touchbar"` survives every rebuild. No entitlement, no boot-arg.
codesign -f -s - -i local.touchbar "$APP"
AGENT="$HOME/Library/LaunchAgents/local.touchbar.plist"
sed "s#__APP__#$APP#" bundle/local.touchbar.plist > "$AGENT"
launchctl bootout "gui/$(id -u)/local.touchbar" 2>/dev/null && sleep 2 || true
launchctl bootstrap "gui/$(id -u)" "$AGENT"
launchctl kickstart "gui/$(id -u)/local.touchbar"
echo "Installed. Log: /tmp/touchbar.log"
