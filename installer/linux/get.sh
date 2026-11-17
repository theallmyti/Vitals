#!/bin/sh
# One-line Vitals install for Linux. Published with every release as "install.sh":
#
#   curl -fsSL https://github.com/theallmyti/Vitals/releases/latest/download/install.sh | sh
#
# It finds the installer for this computer in the latest release, checks it against SHA256SUMS,
# and runs it with sudo. On a first install the installer then shows the pairing QR code.
# Re-running it upgrades in place and keeps the pairing.
set -eu

repo="https://github.com/theallmyti/Vitals/releases/latest/download"

say() { printf '%s\n' "$*"; }
fail() { printf 'Vitals install: %s\n' "$*" >&2; exit 1; }

# Wrapped in main() so a half-downloaded script (curl cut off mid-pipe) runs nothing.
main() {
  [ "$(uname -s)" = Linux ] || fail "this installer is for Linux. Windows: download vitals-setup-*.exe from the release page."

  case "$(uname -m)" in
    x86_64 | amd64) arch=amd64 ;;
    aarch64 | arm64) arch=arm64 ;;
    *) fail "no Vitals build for $(uname -m) (only x86_64 and ARM64)." ;;
  esac

  command -v systemctl >/dev/null 2>&1 || say "Note: systemd not found; Vitals will install but not start on boot."

  if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fsSL --retry 3 -o "$2" "$1"; }
  elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -q -O "$2" "$1"; }
  else
    fail "needs curl or wget."
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    hash_of() { sha256sum "$1" | cut -d' ' -f1; }
  elif command -v shasum >/dev/null 2>&1; then
    hash_of() { shasum -a 256 "$1" | cut -d' ' -f1; }
  else
    fail "needs sha256sum (coreutils) to check the download."
  fi

  if [ "$(id -u)" -eq 0 ]; then
    as_root=""
  elif command -v sudo >/dev/null 2>&1; then
    as_root="sudo"
  else
    fail "run as root, or install sudo."
  fi

  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT

  say "Checking the latest Vitals release..."
  fetch "$repo/SHA256SUMS" "$tmp/SHA256SUMS" || fail "could not reach GitHub."

  # SHA256SUMS is also the list of files: it names the installer for this arch and its checksum.
  line=$(grep -E "^[0-9a-f]{64}  vitals-[0-9.]+-linux-$arch\.run$" "$tmp/SHA256SUMS" | head -1)
  [ -n "$line" ] || fail "the latest release has no Linux $arch installer."
  want=${line%% *}
  file=${line##* }

  say "Downloading $file..."
  fetch "$repo/$file" "$tmp/$file" || fail "download failed."

  got=$(hash_of "$tmp/$file")
  [ "$got" = "$want" ] || fail "checksum mismatch for $file; nothing was installed. Try again."
  say "Checksum OK."
  say

  # This script arrives on stdin, so give the installer the terminal: it asks you to accept the
  # terms and shows the pairing QR code there.
  if [ -r /dev/tty ]; then
    $as_root sh "$tmp/$file" </dev/tty
  else
    say "No terminal to ask for the terms; set ASSUME_YES=1 to accept them for an unattended install."
    [ "${ASSUME_YES:-0}" = 1 ] || fail "not installed."
    $as_root env ASSUME_YES=1 sh "$tmp/$file"
  fi
}

main "$@"
