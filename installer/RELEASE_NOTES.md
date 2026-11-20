## Install

Already have Vitals on a computer? You don't need to do anything: it updates itself. Phones show an update prompt when you open the app.

### Linux (x86-64 and ARM64)

Paste into a terminal:

```sh
curl -fsSL https://github.com/theallmyti/Vitals/releases/latest/download/install.sh | sh
```

It picks the installer for your computer, checks it against `SHA256SUMS`, and asks for your password (sudo) and for you to accept the terms. Then it shows a **QR code**: scan it with the Vitals app to pair. Running the same command again upgrades and keeps the pairing.

<details><summary>Install by hand instead</summary>

Download `vitals-@VERSION@-linux-amd64.run` (or `-arm64.run` on ARM, such as a Raspberry Pi), then:

```sh
sha256sum -c SHA256SUMS --ignore-missing   # optional: should print "OK"
sudo sh vitals-@VERSION@-linux-amd64.run
```
</details>

### Windows 10 and 11

Paste into PowerShell:

```powershell
irm https://github.com/theallmyti/Vitals/releases/latest/download/install.ps1 | iex
```

It downloads the setup, checks it against `SHA256SUMS` and starts it; Windows asks for admin rights. The setup ends on the pairing **QR code**. Or download **`vitals-setup-@VERSION@.exe`** (same file as `vitals-setup.exe`) and run it yourself.

### Android

Download **`vitals-app-@VERSION@.apk`** on the phone and open it. Allow "install unknown apps" for your browser if Android asks.

## Which file do I need?

| File | What it is |
|---|---|
| `vitals-setup-@VERSION@.exe`, `vitals-setup.exe` | Windows installer (the same file; it also repairs, changes settings and uninstalls) |
| `vitals-@VERSION@-linux-amd64.run` | Linux installer, x86-64 PCs |
| `vitals-@VERSION@-linux-arm64.run` | Linux installer, ARM64 (Raspberry Pi 4/5 and similar) |
| `vitals-app-@VERSION@.apk` | Android app |
| `install.sh`, `install.ps1` | The one-line installers above (Linux, Windows) |
| `vitals-uninstall.exe`, `vitals-uninstall.sh` | Uninstallers (Windows, Linux) |
| `SHA256SUMS` | Checksums, to confirm a download isn't corrupted or tampered with. Not something you install. |
| `vitals.exe`, `vitals-linux-*`, `nexus.exe`, `nexus-linux-*` | Used by automatic updates. Don't download these. |

## Uninstall

- **Windows:** Settings › Apps › Vitals › Uninstall.
- **Linux:** `sudo sh /opt/vitals/uninstall.sh`
