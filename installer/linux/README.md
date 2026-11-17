# Vitals Linux Installer & Service

Production installer and hardened systemd service for the Vitals Hardware Monitoring Agent on Linux (`x86_64` and `aarch64` / ARM64).

## Quick Install

```bash
# Standalone run package
sudo sh dist/vitals-<version>-linux-amd64.run   # ARM64: ...-linux-arm64.run
# Or using install.sh directly
sudo sh installer/linux/install.sh [path/to/vitals]
```

## What `install.sh` Does

1. **Binary Placement**: Copies the static musl binary to `/opt/vitals/vitals` (mode 755).
2. **Terminal Command Symlink**: Creates `/usr/local/bin/vitals -> /opt/vitals/vitals` so `vitals` commands run in any shell without specifying the full path.
3. **Secret Security**: Generates `/etc/vitals/config.toml` with `chmod 600` (root-only read/write) to ensure the 256-bit device secret cannot be read by unprivileged users.
4. **Permissive IPC**: Pre-initializes `/etc/vitals/control.json` and `/etc/vitals/status.json` with `chmod 666` so standard users can inspect status and pause/resume metrics collection from user terminals without needing `sudo`.
5. **Interactive Pairing**: Displays an ASCII QR code and 6-character pairing code in the terminal to pair with your mobile app or web dashboard.
6. **Hardened systemd Service**: Deploys and enables `vitals.service`.

## Security Sandboxing (`vitals.service`)

The systemd service enforces strict process isolation:

| Security Setting | Purpose |
|------------------|---------|
| `NoNewPrivileges=yes` | Disallows gaining new privileges via setuid/setgid binaries |
| `ProtectHome=read-only` | Prevents modifying `/home` or `/root` user directories |
| `ProtectSystem=full` | Mounts `/usr`, `/boot`, and `/etc` read-only for the agent |
| `ReadWritePaths=/etc/vitals` | Explicit write-whitelist restricted only to `/etc/vitals` |
| `PrivateTmp=yes` | Isolated `/tmp` and `/var/tmp` directory |
| `ProtectControlGroups=yes` | Read-only control groups hierarchy |
| `ProtectKernelModules=yes` | Prevents kernel module loading/unloading |
| `RestrictRealtime=yes` | Prevents real-time scheduling priority escalation |
| `RestrictSUIDSGID=yes` | Blocks creation of setuid/setgid files |
| `MemoryMax=100M` | Hard memory budget to prevent memory leaks |
| `CPUQuota=10%` | CPU cap to ensure background monitoring never degrades user tasks |

## CLI Terminal Commands

Once installed, standard users and administrators can manage Vitals from the command line:

```bash
# Show live agent status, CPU/RAM %, temperature, and backend link
vitals status

# Temporarily pause metrics collection (default 24h or specify minutes)
vitals pause 30

# Resume live metrics collection
vitals resume

# Restart the background service (via systemctl)
vitals restart

# Pair this machine with your Vitals account
vitals pair

# Remove device secret and unlink from Vitals
vitals unpair

# Inspect detected hardware sensors
vitals sensors

# View command help
vitals help
```

## Uninstallation

```bash
sudo sh /opt/vitals/uninstall.sh
```
This safely disables the systemd service, unpairs the device from the online backend, deletes `/usr/local/bin/vitals`, and purges `/opt/vitals` and `/etc/vitals`.
