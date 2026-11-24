#!/bin/sh
# Installs ostt for the current user. Nothing here needs root.
#   ./install.sh            link the binary, install the service unit (not started) and the app launcher
#   ./install.sh --enable   also start the service now and at every login
#   ./install.sh --remove   undo all of the above (models and settings are kept)
set -eu
cd "$(dirname "$0")"
bin="$HOME/.local/bin/ostt"
unit="$HOME/.config/systemd/user/ostt.service"
entry="$HOME/.local/share/applications/ostt.desktop"

if [ "${1:-}" = "--remove" ]; then
    systemctl --user disable --now ostt.service 2>/dev/null || true
    rm -f "$bin" "$unit" "$entry"
    systemctl --user daemon-reload
    echo "Removed. Models stay in ~/.local/share/ostt, settings in ~/.config/ostt."
    exit 0
fi

[ -x target/release/ostt ] || cargo build --release
mkdir -p "$(dirname "$bin")" "$(dirname "$unit")" "$(dirname "$entry")"
ln -sf "$PWD/target/release/ostt" "$bin"
cp dist/ostt.service "$unit"
sed "s|^Exec=ostt |Exec=$bin |" dist/ostt.desktop > "$entry"
systemctl --user daemon-reload
echo "Installed $bin, $unit and $entry"

if [ "${1:-}" = "--enable" ]; then
    systemctl --user enable --now ostt.service
    echo "Service started and enabled at login."
else
    echo "Start it with: systemctl --user enable --now ostt   (or run: ostt daemon)"
fi
echo "Then bind a key to: $bin toggle"
