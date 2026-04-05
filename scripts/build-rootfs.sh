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
#   ECLIPSE_STRIP_FIRMWARE - If set to 1, remove /usr/lib/firmware for a smaller ISO
#                            (breaks many GPUs/Wi‑Fi). Default: keep firmware for Wayland/KMS.

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
ECLIPSE_STRIP_FIRMWARE="${ECLIPSE_STRIP_FIRMWARE:-0}"

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
#   - dosfstools, e2fsprogs, btrfs-progs (and often xfsprogs/parted) ship in the live rootfs tarball;
#     listing them again makes xbps-install exit with "already installed".
chroot "$ROOTFS" xbps-install -Sy -y \
    linux \
    dbus \
    grub \
    grub-x86_64-efi \
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
    dhcpcd \
    niri \
    seatd \
    mesa \
    mesa-dri \
    mesa-libgallium \
    libgbm \
    libinput \
    xkeyboard-config \
    dejavu-fonts-ttf \
    alacritty \
    fuzzel \
    Waybar \
    xwayland-satellite \
    linux-firmware-amd \
    linux-firmware-intel \
    linux-firmware-nvidia \
    linux-firmware-network

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

# Live ISO uses overlayfs on squashfs; `mount -o remount,rw /` fails on overlay (util-linux
# exits 32) even though the upperdir is writable. That blocked machine-id, dbus, and niri deps.
cat > "$ROOTFS/etc/dynamod/services/remount-root-rw.toml" <<'REMOUNT_RW'
[service]
name = "remount-root-rw"
supervisor = "early-boot"
exec = ["/bin/sh", "-c", "mount -o remount,rw / && exit 0; t=$(findmnt -n -o FSTYPE / 2>/dev/null | head -1); [ \"$t\" = overlay ] && exit 0; exit 1"]
type = "oneshot"

[restart]
policy = "temporary"

[dependencies]
requires = ["fsck"]

[readiness]
type = "none"

[shutdown]
stop-signal = "SIGTERM"
stop-timeout = "3s"
REMOUNT_RW

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

# seatd: libseat provider for niri (LIBSEAT_BACKEND=seatd in profile.d).
# SEATD_VTBOUND=0 on the *daemon* avoids hangs in QEMU/virtio where VT-based seat
# activation never completes; libseat then blocks after loading niri config.
cat > "$ROOTFS/etc/dynamod/services/seatd.toml" <<'SEATD'
[service]
name = "seatd"
supervisor = "root"
exec = ["/bin/sh", "-c", "exec env SEATD_VTBOUND=0 /usr/bin/seatd"]
type = "simple"

[restart]
policy = "permanent"
delay = "1s"
max-restarts = 10
max-restart-window = "60s"

[dependencies]
requires = ["udev"]
after = ["udev-coldplug"]

[readiness]
type = "none"
timeout = "10s"

[shutdown]
stop-signal = "SIGTERM"
stop-timeout = "5s"
SEATD

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

mkdir -p "$ROOTFS/etc/modules-load.d"
printf '%s\n' 'virtio_gpu' > "$ROOTFS/etc/modules-load.d/eclipse-virtio-gpu.conf"

# Install the TUI installer
install -Dm755 "$PROJECT_ROOT/scripts/eclipse-install" "$ROOTFS/usr/bin/eclipse-install"
install -Dm755 "$PROJECT_ROOT/scripts/eclipse-niri-session" "$ROOTFS/usr/bin/eclipse-niri-session"

# Wayland / niri: use seatd (not logind) for libseat
cat > "$ROOTFS/etc/profile.d/eclipse-wayland.sh" <<'WAYLAND'
export LIBSEAT_BACKEND=seatd
if [ -z "${XDG_RUNTIME_DIR}" ]; then
    XDG_RUNTIME_DIR="/run/user/$(id -u)"
    export XDG_RUNTIME_DIR
fi
if [ ! -d "${XDG_RUNTIME_DIR}" ] && [ "$(id -u)" -eq 0 ]; then
    mkdir -m 0700 -p "${XDG_RUNTIME_DIR}" 2>/dev/null || true
fi
WAYLAND
chmod 644 "$ROOTFS/etc/profile.d/eclipse-wayland.sh"

# Shell profile hint for live environment
cat > "$ROOTFS/etc/profile.d/eclipse-live.sh" <<'PROFILE'
if [ -d /run/dynamod/live ]; then
    echo ""
    echo "  Welcome to Eclipse Linux (live environment)"
    echo "  Run 'eclipse-install' to install to disk."
    echo "  Graphical session (tty1): eclipse-niri-session"
    echo "  (or: dbus-run-session niri --session — not plain niri)"
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

# Firmware: keep by default so KMS / niri work on real hardware. Set
# ECLIPSE_STRIP_FIRMWARE=1 for a smaller ISO (console-oriented / VM-only).
if [ "$ECLIPSE_STRIP_FIRMWARE" = "1" ]; then
    echo "  Removing firmware blobs (ECLIPSE_STRIP_FIRMWARE=1)..."
    rm -rf "$ROOTFS/usr/lib/firmware"
    mkdir -p "$ROOTFS/usr/lib/firmware"
else
    echo "  Keeping firmware (unset ECLIPSE_STRIP_FIRMWARE or set to 0 to keep; use =1 to strip)."
fi

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
