#!/bin/sh
# Removes the Vitals agent: stops the service, unpairs the device, deletes program and config.
# Usage: sudo sh /opt/vitals/uninstall.sh (or sudo sh vitals-uninstall.sh)
set -eu
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo: sudo sh $0" >&2; exit 1; }

# Stop process if running standalone or under a non-systemd supervisor
pkill -f /opt/vitals/vitals 2>/dev/null || true
pkill -f /opt/nexus/nexus 2>/dev/null || true
pkill -f /opt/pc-monitor/pc-monitor-agent 2>/dev/null || true

if command -v systemctl >/dev/null 2>&1; then
  systemctl disable --now vitals 2>/dev/null || true
  systemctl disable --now nexus 2>/dev/null || true
  systemctl disable --now pc-monitor-agent 2>/dev/null || true
  rm -f /etc/systemd/system/vitals.service /etc/systemd/system/nexus.service /etc/systemd/system/pc-monitor-agent.service
  systemctl daemon-reload 2>/dev/null || true
fi

# Remove CLI symlinks
rm -f /usr/local/bin/vitals /usr/local/bin/nexus /usr/local/bin/pc-monitor-agent 2>/dev/null || true

if [ -x /opt/vitals/vitals ] && [ -f /etc/vitals/config.toml ]; then
  /opt/vitals/vitals unpair /etc/vitals/config.toml || echo "Could not reach Vitals; remove the device on the site."
elif [ -x /opt/nexus/nexus ] && [ -f /etc/nexus/config.toml ]; then
  # Installed before the rename (Nexus) and never upgraded with the installer.
  /opt/nexus/nexus unpair /etc/nexus/config.toml || echo "Could not reach Vitals; remove the device on the site."
elif [ -x /opt/pc-monitor/pc-monitor-agent ] && [ -f /etc/pc-monitor/config.toml ]; then
  /opt/pc-monitor/pc-monitor-agent unpair /etc/pc-monitor/config.toml || echo "Could not reach Vitals; remove the device on the site."
fi

rm -rf /opt/vitals /etc/vitals /opt/nexus /etc/nexus /opt/pc-monitor /etc/pc-monitor
echo "Vitals agent removed successfully."
