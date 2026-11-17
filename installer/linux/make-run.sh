#!/bin/sh
# Packages the Linux installer as one runnable file, like the Windows setup .exe:
#   dist/vitals-<version>-linux-<amd64|arm64>.run   (users run: sudo sh <file>.run)
#
#   sh installer/linux/make-run.sh <arch> <path/to/vitals>
#
# <arch> is x86_64 or aarch64. The binary comes from the "Linux build" GitHub workflow
# (gh run download -n vitals-linux-<arch>). The Convex URL is read from
# installer/convex-url.txt here, on this computer; it is never uploaded to GitHub.
set -eu

arch=${1:?usage: make-run.sh <x86_64|aarch64> <binary>}
bin=${2:?usage: make-run.sh <x86_64|aarch64> <binary>}
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/agent/Cargo.toml" | head -1)
url_file="$root/installer/convex-url.txt"
[ -f "$url_file" ] || { echo "installer/convex-url.txt is missing" >&2; exit 1; }
[ -f "$bin" ] || { echo "binary not found: $bin" >&2; exit 1; }

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
cp "$bin" "$stage/vitals"
service_src="$here/vitals.service"
cp "$here/install.sh" "$here/uninstall.sh" "$service_src" "$stage/"
tr -d ' \r\n' < "$url_file" > "$stage/convex-url.txt"
chmod 755 "$stage/vitals" "$stage/install.sh" "$stage/uninstall.sh"

# Bundle offline legal and documentation files
mkdir -p "$stage/docs"
[ -f "$root/installer/LEGAL_AGREEMENT.txt" ] && cp "$root/installer/LEGAL_AGREEMENT.txt" "$stage/"
[ -f "$root/LEGAL.md" ] && cp "$root/LEGAL.md" "$stage/"
[ -f "$root/docs/TERMS.md" ] && cp "$root/docs/TERMS.md" "$stage/docs/"
[ -f "$root/docs/PRIVACY.md" ] && cp "$root/docs/PRIVACY.md" "$stage/docs/"
[ -f "$root/docs/COOKIE_POLICY.md" ] && cp "$root/docs/COOKIE_POLICY.md" "$stage/docs/"

# Named amd64/arm64, not x86_64/aarch64: Nexus 0.2.5 agents auto-update from any asset matching
# "*linux-x86_64*.run" and would swap this script in as their binary.
case "$arch" in x86_64) label=amd64 ;; aarch64) label=arm64 ;; *) label=$arch ;; esac
out="$root/dist/vitals-$version-linux-$label.run"
mkdir -p "$root/dist"
cat > "$out" <<EOF
#!/bin/sh
# Vitals agent $version for Linux ($arch, systemd). Run: sudo sh \$(basename "\$out")
set -eu
case "\$(uname -m)" in
  $arch|$( [ "$arch" = aarch64 ] && echo arm64 || echo amd64 )) ;;
  *) echo "This installer is for $arch; this computer is \$(uname -m)." >&2; exit 1 ;;
esac
[ "\$(id -u)" -eq 0 ] || { echo "Run with sudo: sudo sh \$0" >&2; exit 1; }
tmp=\$(mktemp -d)
trap 'rm -rf "\$tmp"' EXIT
line=\$(awk '/^__PAYLOAD_BELOW__\$/ { print NR + 1; exit }' "\$0")
tail -n +"\$line" "\$0" | tar -xzf - -C "\$tmp"
chmod 755 "\$tmp/vitals" # file modes are not kept when packaged on Windows
sh "\$tmp/install.sh" "\$tmp/vitals"
exit 0
__PAYLOAD_BELOW__
EOF
tar -czf - -C "$stage" . >> "$out"
chmod 755 "$out"
echo "Built $out ($(wc -c < "$out") bytes)"

# Raw binaries for the release, used by auto-update. nexus-linux-<arch> is the same file under its
# pre-rename name: Nexus agents only look for that, so it is how they update to Vitals.
cp "$bin" "$root/dist/vitals-linux-$arch"
cp "$bin" "$root/dist/nexus-linux-$arch"
chmod 755 "$root/dist/vitals-linux-$arch" "$root/dist/nexus-linux-$arch"
echo "Copied auto-update binaries vitals-linux-$arch and nexus-linux-$arch to dist/"

cp "$here/uninstall.sh" "$root/dist/vitals-uninstall.sh"
chmod 755 "$root/dist/vitals-uninstall.sh"
echo "Generated $root/dist/vitals-uninstall.sh"
