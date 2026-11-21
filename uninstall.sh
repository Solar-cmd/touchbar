#!/bin/sh
launchctl bootout "gui/$(id -u)/local.touchbar" 2>/dev/null || true
rm -f "$HOME/Library/LaunchAgents/local.touchbar.plist"
rm -rf "$HOME/Applications/TouchBar.app"
echo "Removed."
