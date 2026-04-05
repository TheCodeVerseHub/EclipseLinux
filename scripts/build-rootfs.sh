#!/bin/sh
# Build the Eclipse Linux root filesystem from a Void Linux musl base.
#
# Downloads the Void musl rootfs tarball, installs packages via xbps in a
# chroot, installs dynamod binaries and configs, applies Eclipse branding,
# and prepares the rootfs for ISO packing.
#
# Usage:
#   sudo scripts/build-rootfs.sh [build-dir]
#
# Environment:
#   VOID_DATE       - Void rootfs tarball date stamp (default: 20250202)
#   ECLIPSE_VERSION - Version string (default: 0.1.0)

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="${1:-$PROJECT_ROOT/build}"
ROOTFS="$BUILD_DIR/rootfs"
CACHE_DIR="$BUILD_DIR/.cache"

VOID_DATE="${VOID_DATE:-20250202}"
VOID_ARCH="x86_64"
VOID_LIBC="musl"
VOID_MIRROR="https://repo-default.voidlinux.org/live/current"
VOID_ROOTFS="void-${VOID_ARCH}-${VOID_LIBC}-ROOTFS-${VOID_DATE}.tar.xz"
VOID_URL="${VOID_MIRROR}/${VOID_ROOTFS}"

ECLIPSE_VERSION="${ECLIPSE_VERSION:-0.1.0}"

DYNAMOD_DIR="$PROJECT_ROOT/dynamod"
ZIG_OUT="$DYNAMOD_DIR/zig/zig-out/bin"
CARGO_OUT="$(if [ -d "$DYNAMOD_DIR/rust/target/x86_64-unknown-linux-musl/release" ]; then
    echo "$DYNAMOD_DIR/rust/target/x86_64-unknown-linux-musl/release"
else
    echo "$DYNAMOD_DIR/rust/target/release"
fi)"

# ============================================================
# Checks
# ============================================================
echo "=== Eclipse Linux Rootfs Builder ==="
echo "Output:  $ROOTFS"
echo "Version: $ECLIPSE_VERSION"
echo ""

if [ "$(id -u)" -ne 0 ]; then
    echo "ERROR: This script requires root (for chroot, mount)."
    echo "  Run: sudo $0 $*"
    exit 1
fi

for bin in "$ZIG_OUT/dynamod-init" \
           "$CARGO_OUT/dynamod-svmgr" \
           "$CARGO_OUT/dynamodctl" \
           "$CARGO_OUT/dynamod-logd"; do
    if [ ! -f "$bin" ]; then
        echo "ERROR: $bin not found. Run 'make dynamod' first."
        exit 1
    fi
done

# ============================================================
# Download Void rootfs
# ============================================================
mkdir -p "$CACHE_DIR"
if [ ! -f "$CACHE_DIR/$VOID_ROOTFS" ]; then
    echo "Downloading Void Linux rootfs ($VOID_ROOTFS)..."
    echo "  URL: $VOID_URL"
    if command -v wget >/dev/null 2>&1; then
        wget -q --show-progress -O "$CACHE_DIR/$VOID_ROOTFS.part" "$VOID_URL" || {
            rm -f "$CACHE_DIR/$VOID_ROOTFS.part"
            echo "ERROR: Download failed. Check VOID_DATE=$VOID_DATE against"
            echo "  https://repo-default.voidlinux.org/live/current/"
            exit 1
        }
    elif command -v curl >/dev/null 2>&1; then
        curl -fL --progress-bar -o "$CACHE_DIR/$VOID_ROOTFS.part" "$VOID_URL" || {
            rm -f "$CACHE_DIR/$VOID_ROOTFS.part"
            echo "ERROR: Download failed. Check VOID_DATE=$VOID_DATE against"
            echo "  https://repo-default.voidlinux.org/live/current/"
            exit 1
        }
    else
        echo "ERROR: wget or curl required"
        exit 1
    fi
    mv "$CACHE_DIR/$VOID_ROOTFS.part" "$CACHE_DIR/$VOID_ROOTFS"
fi

# ============================================================
# Extract rootfs
# ============================================================
if [ -d "$ROOTFS" ]; then
    echo "Removing old rootfs..."
    rm -rf "$ROOTFS"
fi

mkdir -p "$ROOTFS"
echo "Extracting Void rootfs..."
tar xJf "$CACHE_DIR/$VOID_ROOTFS" -C "$ROOTFS"

# ============================================================
# Chroot: install packages
# ============================================================
echo "Setting up chroot..."
cp /etc/resolv.conf "$ROOTFS/etc/resolv.conf" 2>/dev/null || \
    echo "nameserver 8.8.8.8" > "$ROOTFS/etc/resolv.conf"

mount --bind /proc "$ROOTFS/proc"
mount --bind /dev  "$ROOTFS/dev"
mount --bind /sys  "$ROOTFS/sys"

chroot_cleanup() {
    set +e
    umount "$ROOTFS/proc" 2>/dev/null
    umount "$ROOTFS/dev"  2>/dev/null
    umount "$ROOTFS/sys"  2>/dev/null
}
trap chroot_cleanup EXIT

echo "Updating xbps..."
chroot "$ROOTFS" xbps-install -Syu xbps -y

echo "Installing packages..."
# Notes:
#   - 'grub' on Void includes i386-pc modules (no separate grub-i386-pc package)
#   - eudev is already in the Void musl rootfs, so skip it to avoid "already installed" errors
chroot "$ROOTFS" xbps-install -Sy -y \
    linux \
    dbus \
    grub \
    grub-x86_64-efi \
    dosfstools \
    e2fsprogs \
    btrfs-progs \
    xfsprogs \
    parted \
    dialog \
    rsync \
    kmod \
    iproute2 \
    iputils \
    kbd \
    tzdata \
    util-linux \
    busybox \
    ca-certificates \
    dhcpcd

# ============================================================
# Install dynamod binaries
# ============================================================
echo "Installing dynamod binaries..."
install -Dm755 "$ZIG_OUT/dynamod-init"       "$ROOTFS/sbin/dynamod-init"
install -Dm755 "$CARGO_OUT/dynamod-svmgr"    "$ROOTFS/usr/lib/dynamod/dynamod-svmgr"
install -Dm755 "$CARGO_OUT/dynamodctl"        "$ROOTFS/usr/bin/dynamodctl"
install -Dm755 "$CARGO_OUT/dynamod-logd"      "$ROOTFS/usr/lib/dynamod/dynamod-logd"

for bin in dynamod-logind dynamod-sd1bridge dynamod-hostnamed; do
    if [ -f "$CARGO_OUT/$bin" ]; then
        install -Dm755 "$CARGO_OUT/$bin" "$ROOTFS/usr/lib/dynamod/$bin"
    fi
done

# ============================================================
# Install dynamod configs
# ============================================================
echo "Installing dynamod service configs..."
mkdir -p "$ROOTFS/etc/dynamod/services" "$ROOTFS/etc/dynamod/supervisors"

cp "$DYNAMOD_DIR/config/supervisors/"*.toml "$ROOTFS/etc/dynamod/supervisors/"

for svc in fsck remount-root-rw machine-id fstab-mount modules-load \
           bootmisc hostname network sysctl dynamod-logd \
           udev udev-coldplug \
           dynamod-logind dynamod-sd1bridge dynamod-hostnamed; do
    if [ -f "$DYNAMOD_DIR/config/services/${svc}.toml" ]; then
        cp "$DYNAMOD_DIR/config/services/${svc}.toml" "$ROOTFS/etc/dynamod/services/"
    fi
done

# D-Bus policy files
mkdir -p "$ROOTFS/usr/share/dbus-1/system.d"
cp "$DYNAMOD_DIR/config/dbus-1/"*.conf "$ROOTFS/usr/share/dbus-1/system.d/" 2>/dev/null || true

# Eclipse-specific overrides: dbus service with /run/dbus mkdir
cat > "$ROOTFS/etc/dynamod/services/dbus.toml" <<'DBUS'
[service]
name = "dbus"
supervisor = "root"
exec = ["/bin/sh", "-c", "mkdir -p /run/dbus && exec /usr/bin/dbus-daemon --system --nofork --nopidfile"]
type = "simple"

[restart]
policy = "permanent"
delay = "1s"
max-restarts = 10
max-restart-window = "60s"

[readiness]
type = "none"
timeout = "10s"

[dependencies]
requires = ["bootmisc", "machine-id"]

[shutdown]
stop-signal = "SIGTERM"
stop-timeout = "5s"
DBUS

# agetty-tty1: use Void's agetty (from util-linux), depend on udev-coldplug
cat > "$ROOTFS/etc/dynamod/services/agetty-tty1.toml" <<'GETTY'
[service]
name = "agetty-tty1"
exec = ["/usr/bin/agetty", "tty1", "38400", "linux"]
type = "simple"

[restart]
policy = "permanent"
delay = "1s"
max-restarts = 10
max-restart-window = "30s"

[dependencies]
after = ["bootmisc", "udev-coldplug"]

[readiness]
type = "none"

[shutdown]
stop-signal = "SIGHUP"
stop-timeout = "3s"
GETTY

# Serial console getty (for QEMU testing)
cat > "$ROOTFS/etc/dynamod/services/agetty-ttyS0.toml" <<'GETTY_S'
[service]
name = "agetty-ttyS0"
exec = ["/usr/bin/agetty", "ttyS0", "115200", "vt100"]
type = "simple"

[restart]
policy = "permanent"
delay = "1s"
max-restarts = 10
max-restart-window = "30s"

[dependencies]
after = ["bootmisc"]

[readiness]
type = "none"

[shutdown]
stop-signal = "SIGHUP"
stop-timeout = "3s"
GETTY_S

# DHCP service for live environment networking
cat > "$ROOTFS/etc/dynamod/services/dhcpcd.toml" <<'DHCP'
[service]
name = "dhcpcd"
supervisor = "root"
exec = ["/usr/sbin/dhcpcd", "--nobackground", "-f", "/etc/dhcpcd.conf"]
type = "simple"

[restart]
policy = "permanent"
delay = "2s"
max-restarts = 5
max-restart-window = "60s"

[dependencies]
after = ["network", "udev-coldplug"]

[readiness]
type = "none"

[shutdown]
stop-signal = "SIGTERM"
stop-timeout = "10s"
DHCP

# Permissive D-Bus system.conf for dynamod mimic daemons
mkdir -p "$ROOTFS/etc/dbus-1"
cat > "$ROOTFS/etc/dbus-1/system.conf" <<'DBUSCONF'
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
  "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>system</type>
  <listen>unix:path=/run/dbus/system_bus_socket</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
    <allow send_type="method_call"/>
    <allow send_type="signal"/>
  </policy>
  <includedir>system.d</includedir>
  <includedir>/usr/share/dbus-1/system.d</includedir>
</busconfig>
DBUSCONF

# ============================================================
# System configuration
# ============================================================
echo "Configuring Eclipse Linux..."

# Branding
cp "$PROJECT_ROOT/config/os-release" "$ROOTFS/etc/os-release"
cp "$PROJECT_ROOT/config/motd"       "$ROOTFS/etc/motd"
cp "$PROJECT_ROOT/config/issue"      "$ROOTFS/etc/issue"

echo "eclipse" > "$ROOTFS/etc/hostname"
cat > "$ROOTFS/etc/hosts" <<'HOSTS'
127.0.0.1   localhost
::1         localhost
127.0.1.1   eclipse
HOSTS

# Empty root password for live environment
sed -i 's|^root:.*|root::0:0:root:/root:/bin/sh|' "$ROOTFS/etc/passwd"

# /var/run -> /run symlink (required by D-Bus/elogind)
rm -rf "$ROOTFS/var/run" 2>/dev/null
ln -sf /run "$ROOTFS/var/run"

# Required directories
mkdir -p "$ROOTFS/var/log/dynamod" \
         "$ROOTFS/var/lib/dynamod" \
         "$ROOTFS/run" \
         "$ROOTFS/tmp" \
         "$ROOTFS/proc" \
         "$ROOTFS/sys" \
         "$ROOTFS/dev"
chmod 1777 "$ROOTFS/tmp"

# Minimal fstab for live (installer will replace on install)
cat > "$ROOTFS/etc/fstab" <<'FSTAB'
# Eclipse Linux
# This fstab is for the live environment. The installer will generate
# a proper fstab during installation.
FSTAB

touch "$ROOTFS/etc/modules" "$ROOTFS/etc/sysctl.conf"

# Install the TUI installer
install -Dm755 "$PROJECT_ROOT/scripts/eclipse-install" "$ROOTFS/usr/bin/eclipse-install"

# Shell profile hint for live environment
cat > "$ROOTFS/etc/profile.d/eclipse-live.sh" <<'PROFILE'
if [ -d /run/dynamod/live ]; then
    echo ""
    echo "  Welcome to Eclipse Linux (live environment)"
    echo "  Run 'eclipse-install' to install to disk."
    echo ""
fi
PROFILE
chmod 644 "$ROOTFS/etc/profile.d/eclipse-live.sh"

# ============================================================
# Cleanup
# ============================================================
echo "Cleaning up chroot mounts..."
umount "$ROOTFS/proc" 2>/dev/null || true
umount "$ROOTFS/dev"  2>/dev/null || true
umount "$ROOTFS/sys"  2>/dev/null || true
trap - EXIT

rm -f "$ROOTFS/etc/resolv.conf"
rm -rf "$ROOTFS/var/cache/xbps"

# ============================================================
# Ensure GRUB i386-pc modules exist for the installer
# ============================================================
# Void's grub package may not include i386-pc modules. Bundle them
# from the build host so grub-install works during installation.
if [ ! -f "$ROOTFS/usr/lib/grub/i386-pc/normal.mod" ]; then
    echo "GRUB i386-pc modules not in rootfs, bundling from host..."
    for d in /usr/lib/grub/i386-pc /usr/share/grub/i386-pc; do
        if [ -d "$d" ] && [ -f "$d/normal.mod" ]; then
            mkdir -p "$ROOTFS/usr/lib/grub/i386-pc"
            cp "$d/"*.mod "$ROOTFS/usr/lib/grub/i386-pc/"
            cp "$d/"*.lst "$ROOTFS/usr/lib/grub/i386-pc/" 2>/dev/null || true
            cp "$d/"*.img "$ROOTFS/usr/lib/grub/i386-pc/" 2>/dev/null || true
            echo "  Copied from $d"
            break
        fi
    done
fi

# ============================================================
# Slim down rootfs for live ISO (~700 MB savings)
# ============================================================
echo "Slimming rootfs for live ISO..."

# Remove linux-firmware blobs (643 MB); wired NICs and storage controllers
# work without firmware. WiFi/GPU firmware can be added back later.
echo "  Removing firmware blobs..."
rm -rf "$ROOTFS/usr/lib/firmware"
mkdir -p "$ROOTFS/usr/lib/firmware"

# Remove non-English locales (53 MB)
echo "  Stripping locales..."
if [ -d "$ROOTFS/usr/share/locale" ]; then
    find "$ROOTFS/usr/share/locale" -mindepth 1 -maxdepth 1 \
        ! -name 'en*' ! -name 'C' ! -name 'POSIX' -exec rm -rf {} + 2>/dev/null || true
fi

# Remove man/info pages (22 MB)
echo "  Removing man/info pages..."
rm -rf "$ROOTFS/usr/share/man" "$ROOTFS/usr/share/info"

# Remove kernel headers and module build artifacts
rm -rf "$ROOTFS"/usr/src/kernel-headers-*
find "$ROOTFS/lib/modules" -name 'build' -type l -delete 2>/dev/null || true
find "$ROOTFS/lib/modules" -name 'source' -type l -delete 2>/dev/null || true

echo ""
echo "=== Rootfs Build Complete ==="
echo "  $ROOTFS ($(du -sh "$ROOTFS" | cut -f1))"
