#!/bin/sh -e

echo "-- TLM Snap Auto-Installer --"
echo
echo "-----"
echo "IMPORTANT:"
echo "Snap is considered an unsupported and experimental install type. TLM might work fine, or it may not work at all."
echo "If you run into issues, you should consider installing Steam a different way, like from a native package."
echo "------"
echo

if [ "$(id -u)" -eq 0 ]; then
    echo 'This script cannot be ran as the root user or with sudo. Please run it as a regular user.'
    exit 1
fi

echo "[Step: 1] Downloading TLM"
curl --fail -L https://github.com/3ventic/tlm/releases/latest/download/tlm-x86_64-unknown-linux-gnu > /tmp/tlm

echo "[Step: 2] Configuring TLM as a Steam Tool"
chmod +x /tmp/tlm
/tmp/tlm install-steam-tool --steam-compat-path ~/snap/steam/common/.steam/root/compatibilitytools.d/

echo "[Step: 3] Cleanup TLM binary"
rm /tmp/tlm

echo
echo "-- Auto Installer Complete: Restart Steam and follow the guide at https://github.com/3ventic/tlm#readme to continue! --"