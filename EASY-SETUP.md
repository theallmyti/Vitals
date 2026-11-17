# Easy Setup Guide — Vitals Agent

This guide covers running and pairing the Vitals PC Monitoring Agent.

---

## Quick Start (Automated Installer)

1. **Download**: Obtain `vitals-setup-<version>.exe` from GitHub Releases.
2. **Run**: Double-click `vitals-setup-<version>.exe` to automatically install and launch the agent.
3. **Pair**:
   - The agent displays a 6-character pairing code in the console / system tray.
   - Open the **Vitals Android App**.
   - Tap **Add Device** → Enter the 6-character code (or scan the QR code).
   - Your PC will appear in your device list with live telemetry.

---

## Service Management
- **Start Agent**: Double-click `Start-PC-Monitor.bat` or run `vitals.exe` / `pc-monitor-agent.exe`.
- **Stop Agent**: Right-click the tray icon and select **Exit**, or double-click `Stop-PC-Monitor.bat`.
