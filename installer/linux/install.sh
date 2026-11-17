#!/bin/sh
# Vitals agent installer for Linux (x86-64 and ARM64, systemd).
#
#   sudo sh install.sh [path/to/vitals]
#
# Binary: the argument, else ./vitals next to this script, else agent/target/release/vitals.
# Convex URL: $CONVEX_URL, else convex-url.txt next to this script (kept out of the repo).
# Steps: copy the binary to /opt/vitals, create /usr/local/bin/vitals symlink for CLI terminal commands,
# configure /etc/vitals/config.toml, initialize IPC control files, pair, and enable systemd service.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo: sudo sh $0" >&2; exit 1; }
has_systemd=0
if command -v systemctl >/dev/null 2>&1; then
  has_systemd=1
fi

bin=${1:-}
[ -n "$bin" ] || for c in "$here/vitals" "$here/../../agent/target/release/vitals" "$here/../../agent/target/x86_64-unknown-linux-musl/release/vitals" "$here/../../agent/target/aarch64-unknown-linux-musl/release/vitals" "$here/pc-monitor-agent"; do
  [ -f "$c" ] && { bin=$c; break; }
done
[ -n "$bin" ] && [ -f "$bin" ] || { echo "Agent binary not found. Pass its path: sudo sh $0 /path/to/vitals" >&2; exit 1; }

url=${CONVEX_URL:-}
[ -n "$url" ] || { [ -f "$here/convex-url.txt" ] && url=$(tr -d ' \r\n' < "$here/convex-url.txt"); }
case "$url" in https://*.convex.cloud) ;; *) echo "Set CONVEX_URL or put the Convex URL in $here/convex-url.txt" >&2; exit 1 ;; esac

echo "================================================================================"
echo "                     VITALS END USER TERMS & PRIVACY NOTICE"
echo "================================================================================"
echo "Vitals is open-source hardware monitoring software licensed under the MIT License."
echo "By installing Vitals on this system, you confirm that:"
echo "  1. You are at least 18 years of age."
echo "  2. You own this computer or have explicit permission to monitor its hardware."
echo "     Unauthorized surveillance of individuals is strictly prohibited."
echo "  3. You accept the Vitals Terms of Service and Privacy Policy:"
echo "     https://github.com/theallmyti/Vitals/blob/main/docs/TERMS.md"
echo "     https://github.com/theallmyti/Vitals/blob/main/docs/PRIVACY.md"
echo "================================================================================"
echo

if [ "${ASSUME_YES:-0}" -ne 1 ] && [ -t 0 ]; then
  printf "Do you accept the Terms of Service and Privacy Policy? [y/N]: "
  read -r accept
  case "$accept" in
    [yY]|[yY][eE][sS]) ;;
    *) echo "Installation cancelled: Terms of Service not accepted." >&2; exit 1 ;;
  esac
fi

if [ "$has_systemd" -eq 1 ]; then
  systemctl stop vitals 2>/dev/null || true
  systemctl stop nexus 2>/dev/null || true
  systemctl stop pc-monitor-agent 2>/dev/null || true
fi
install -d -m 755 /opt/vitals
install -m 755 "$bin" /opt/vitals/vitals
[ -f "$here/uninstall.sh" ] && install -m 755 "$here/uninstall.sh" /opt/vitals/uninstall.sh

# Documentation and legal terms
install -d -m 755 /opt/vitals/docs
[ -f "$here/LEGAL_AGREEMENT.txt" ] && install -m 644 "$here/LEGAL_AGREEMENT.txt" /opt/vitals/docs/LEGAL_AGREEMENT.txt
[ -f "$here/LEGAL.md" ] && install -m 644 "$here/LEGAL.md" /opt/vitals/docs/LEGAL.md
[ -f "$here/docs/TERMS.md" ] && install -m 644 "$here/docs/TERMS.md" /opt/vitals/docs/TERMS.md
[ -f "$here/docs/PRIVACY.md" ] && install -m 644 "$here/docs/PRIVACY.md" /opt/vitals/docs/PRIVACY.md
[ -f "$here/docs/COOKIE_POLICY.md" ] && install -m 644 "$here/docs/COOKIE_POLICY.md" /opt/vitals/docs/COOKIE_POLICY.md

# Symlink so `vitals` terminal commands work anywhere without full path
ln -sf /opt/vitals/vitals /usr/local/bin/vitals
rm -f /usr/local/bin/nexus /usr/local/bin/pc-monitor-agent 2>/dev/null || true

# Configuration directory and files
install -d -m 755 /etc/vitals
cfg=/etc/vitals/config.toml
# Upgrade from Nexus (/etc/nexus) or PC Monitor (/etc/pc-monitor): keep the device secret, so the
# computer stays paired with its history.
for legacy_cfg in /etc/nexus/config.toml /etc/pc-monitor/config.toml; do
  if [ ! -f "$cfg" ] && [ -f "$legacy_cfg" ]; then
    (umask 077; cp "$legacy_cfg" "$cfg")
  fi
done

if [ -f "$cfg" ]; then
  # Upgrade: keep the device secret, point at this build's backend.
  sed -i "s|^convex_url *=.*|convex_url = \"$url\"|" "$cfg"
else
  umask 077
  printf '# Vitals agent config, written by install.sh.\nconvex_url = "%s"\nlive_interval_secs = 1\n' "$url" > "$cfg"
fi
chmod 600 "$cfg"

# Initialize IPC control and status files so standard users can inspect status and pause/resume
touch /etc/vitals/control.json /etc/vitals/status.json 2>/dev/null || true
chmod 666 /etc/vitals/control.json /etc/vitals/status.json 2>/dev/null || true

echo
echo "Pair this computer: scan the QR code below with your camera or Google Lens to open Vitals (or download the APK),"
echo "or type the 6-character code in the app. Press Ctrl+C to skip; the agent then waits until it is paired."
echo
/opt/vitals/vitals pair "$cfg" || echo "Not paired yet. Pair later with: sudo vitals pair"

if [ "$has_systemd" -eq 1 ]; then
  service_src="$here/vitals.service"
  install -m 644 "$service_src" /etc/systemd/system/vitals.service
  systemctl disable --now nexus 2>/dev/null || true
  systemctl disable --now pc-monitor-agent 2>/dev/null || true
  rm -f /etc/systemd/system/nexus.service /etc/systemd/system/pc-monitor-agent.service
  systemctl daemon-reload
  systemctl enable --now vitals
  echo
  echo "Installed and started Vitals background service."
  echo "Service status: systemctl status vitals"
  echo "Live logs:      journalctl -u vitals -f"
else
  echo
  echo "Installed to /opt/vitals/vitals."
  echo "Systemd not detected on this machine. To run the agent: sudo vitals"
fi

# Remove the Nexus install this one replaces, now that its config is copied. Not its uninstall.sh:
# that would unpair the device secret this install has just taken over.
if [ -f "$cfg" ]; then
  pkill -f /opt/nexus/nexus 2>/dev/null || true
  rm -rf /opt/nexus /etc/nexus
fi

echo
echo "Vitals CLI Terminal Commands:"
echo "  vitals status       - View real-time status, CPU/RAM utilization, and temperature"
echo "  vitals pause [MINS] - Temporarily pause monitoring (default: 24h, or specify minutes)"
echo "  vitals resume       - Resume live monitoring immediately"
echo "  vitals restart      - Restart the background agent service"
echo "  vitals pair         - Pair this machine with your Vitals account"
echo "  vitals unpair       - Unlink this machine from Vitals"
echo "  vitals sensors      - List detected hardware sensors"
echo
echo "Uninstall: sudo sh /opt/vitals/uninstall.sh"
