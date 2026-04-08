#!/bin/sh
# Build an Eclipse Linux hybrid ISO (BIOS + UEFI) from a populated rootfs.
#
# Takes the rootfs directory produced by build-rootfs.sh, extracts the
# kernel, builds a minimal initramfs, packs rootfs into squashfs, and
# assembles a bootable ISO with GRUB for both BIOS and UEFI boot.
#
# Usage:
#   sudo scripts/build-iso.sh [build-dir]
#
# Environment:
#   ECLIPSE_VERSION - Version string for ISO filename (default: 0.1.0)

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="${1:-$PROJECT_ROOT/build}"
ROOTFS="$BUILD_DIR/rootfs"
WORK="$BUILD_DIR/iso-work"
ISO_DIR="$WORK/iso"
ECLIPSE_VERSION="${ECLIPSE_VERSION:-0.1.0}"
ISO_LABEL="ECLIPSE"
ISO_FILE="$BUILD_DIR/eclipse-linux-${ECLIPSE_VERSION}.iso"

DYNAMOD_DIR="$PROJECT_ROOT/dynamod"
ZIG_OUT="$DYNAMOD_DIR/zig/zig-out/bin"

echo "=== Eclipse Linux ISO Builder ==="
echo "Rootfs:  $ROOTFS"
echo "Output:  $ISO_FILE"
echo ""

# ============================================================
# Checks
# ============================================================
if [ "$(id -u)" -ne 0 ]; then
    echo "ERROR: This script requires root."
    exit 1
fi

if [ ! -d "$ROOTFS/sbin" ]; then
    echo "ERROR: $ROOTFS does not look like a rootfs. Run build-rootfs.sh first."
    exit 1
fi

for cmd in mksquashfs xorriso grub-mkimage grub-mkstandalone mformat cpio gzip; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "ERROR: $cmd not found. Install required packages."
        echo "  On Void: xbps-install -S squashfs-tools xorriso grub grub-x86_64-efi mtools cpio"
        echo "  On Arch: pacman -S squashfs-tools xorriso grub mtools cpio"
        exit 1
    fi
done

# ============================================================
# Find kernel in rootfs
# ============================================================
KERNEL=""
KVER=""
for k in "$ROOTFS"/boot/vmlinuz-*; do
    if [ -f "$k" ]; then
        KERNEL="$k"
        KVER="$(basename "$k" | sed 's/^vmlinuz-//')"
        break
    fi
done

if [ -z "$KERNEL" ]; then
    echo "ERROR: No kernel found in $ROOTFS/boot/. Was the 'linux' package installed?"
    exit 1
fi

echo "Kernel:  $KERNEL (version: $KVER)"

# ============================================================
# Clean work directory
# ============================================================
rm -rf "$WORK"
mkdir -p "$ISO_DIR/boot/grub/i386-pc" \
         "$ISO_DIR/boot/grub/x86_64-efi" \
         "$ISO_DIR/EFI/BOOT" \
         "$ISO_DIR/live"

# ============================================================
# Build initramfs
# ============================================================
echo "Building initramfs..."
INITRAMFS_DIR="$WORK/initramfs"
mkdir -p "$INITRAMFS_DIR/sbin" \
         "$INITRAMFS_DIR/bin" \
         "$INITRAMFS_DIR/dev" \
         "$INITRAMFS_DIR/proc" \
         "$INITRAMFS_DIR/sys" \
         "$INITRAMFS_DIR/newroot" \
         "$INITRAMFS_DIR/etc" \
         "$INITRAMFS_DIR/lib"

# dynamod-init as PID 1
cp "$ZIG_OUT/dynamod-init" "$INITRAMFS_DIR/sbin/dynamod-init"

# Busybox for device detection, mounting, module loading
BUSYBOX=""
for bb in "$ROOTFS/usr/bin/busybox" "$ROOTFS/bin/busybox" \
          "$(command -v busybox 2>/dev/null)"; do
    if [ -f "$bb" ]; then
        BUSYBOX="$bb"
        break
    fi
done

if [ -n "$BUSYBOX" ]; then
    cp "$BUSYBOX" "$INITRAMFS_DIR/bin/busybox"
    for cmd in sh mount umount losetup blkid switch_root; do
        ln -sf busybox "$INITRAMFS_DIR/bin/$cmd"
    done
    echo "  Included busybox for initramfs helpers"
else
    echo "  WARNING: No busybox found"
fi

# Use kmod from rootfs for modprobe (busybox modprobe can't load .ko.zst modules).
# kmod is a musl-linked dynamic binary; we must copy the full musl dependency chain.
KMOD=""
for km in "$ROOTFS/usr/bin/kmod" "$ROOTFS/bin/kmod"; do
    [ -f "$km" ] && KMOD="$km" && break
done
if [ -n "$KMOD" ]; then
    cp "$KMOD" "$INITRAMFS_DIR/bin/kmod"
    ln -sf kmod "$INITRAMFS_DIR/bin/modprobe"
    ln -sf kmod "$INITRAMFS_DIR/bin/insmod"
    ln -sf kmod "$INITRAMFS_DIR/bin/lsmod"
    ln -sf ../bin/modprobe "$INITRAMFS_DIR/sbin/modprobe"

    # Copy the musl dynamic linker (ld-musl-x86_64.so.1 -> /usr/lib64/libc.so)
    mkdir -p "$INITRAMFS_DIR/lib" "$INITRAMFS_DIR/usr/lib64" "$INITRAMFS_DIR/usr/lib"
    for ld in "$ROOTFS"/lib/ld-musl-*.so*; do
        [ -e "$ld" ] || continue
        cp -a "$ld" "$INITRAMFS_DIR/lib/"
    done
    # The musl libc itself (the linker symlink target)
    for libc in "$ROOTFS"/usr/lib64/libc.so "$ROOTFS"/usr/lib/libc.so "$ROOTFS"/lib/libc.so; do
        [ -f "$libc" ] || continue
        cp -a "$libc" "$INITRAMFS_DIR$(echo "$libc" | sed "s|^$ROOTFS||")"
    done

    # Shared libs kmod links against (zstd, lzma, crypto, etc.)
    for pattern in libzstd liblzma libcrypto libz; do
        for lib in "$ROOTFS"/usr/lib/${pattern}.so* "$ROOTFS"/lib/${pattern}.so* \
                   "$ROOTFS"/usr/lib64/${pattern}.so*; do
            [ -e "$lib" ] || continue
            dest="$INITRAMFS_DIR$(echo "$lib" | sed "s|^$ROOTFS||")"
            mkdir -p "$(dirname "$dest")"
            cp -a "$lib" "$dest"
        done
    done
    echo "  Included kmod + musl libc + zstd/lzma libs for module loading"
else
    ln -sf busybox "$INITRAMFS_DIR/bin/modprobe"
    ln -sf ../bin/modprobe "$INITRAMFS_DIR/sbin/modprobe"
    echo "  WARNING: kmod not found, using busybox modprobe (may not load .ko.zst modules)"
fi

# Module hints for modular kernels
cat > "$INITRAMFS_DIR/etc/modules" <<'MODS'
scsi_mod
ata_piix
cdrom
sr_mod
squashfs
loop
iso9660
udf
overlay
virtio_blk
virtio_scsi
virtio_pci
ahci
nvme
xhci_hcd
usb_storage
ext4
MODS

# Bundle the full kernel module tree. Dependency resolution for selective
# copying is fragile (transitive deps like scsi_common, libata, virtio_ring
# are easily missed). The real size savings come from stripping firmware
# from the squashfs rootfs, not from the initramfs modules.
if [ -d "$ROOTFS/lib/modules/$KVER" ]; then
    echo "  Bundling kernel modules ($KVER)..."
    mkdir -p "$INITRAMFS_DIR/lib/modules"
    cp -a "$ROOTFS/lib/modules/$KVER" "$INITRAMFS_DIR/lib/modules/$KVER"
    echo "  Included $(du -sh "$INITRAMFS_DIR/lib/modules/$KVER" | cut -f1) of kernel modules"
fi

# Pack initramfs
cd "$INITRAMFS_DIR"
find . -print0 | cpio --null -o --format=newc 2>/dev/null | gzip -9 > "$WORK/initramfs.gz"
cd "$PROJECT_ROOT"
echo "  Initramfs: $(du -sh "$WORK/initramfs.gz" | cut -f1)"

# ============================================================
# Copy kernel and initramfs to ISO
# ============================================================
cp "$KERNEL" "$ISO_DIR/boot/vmlinuz"
cp "$WORK/initramfs.gz" "$ISO_DIR/boot/initramfs.gz"

# ============================================================
# GRUB configuration
# ============================================================
cp "$PROJECT_ROOT/config/grub-live.cfg" "$ISO_DIR/boot/grub/grub.cfg"

# ============================================================
# Build squashfs
# ============================================================
echo "Creating squashfs (this may take a few minutes)..."
mksquashfs "$ROOTFS" "$ISO_DIR/live/root.squashfs" \
    -comp xz \
    -noappend \
    -no-progress 2>/dev/null
echo "  Squashfs: $(du -sh "$ISO_DIR/live/root.squashfs" | cut -f1)"

# ============================================================
# GRUB BIOS boot image
# ============================================================
echo "Building GRUB BIOS boot image..."

BIOS_MOD_DIR=""
for d in /usr/lib/grub/i386-pc /usr/share/grub/i386-pc "$ROOTFS/usr/lib/grub/i386-pc"; do
    if [ -d "$d" ]; then
        BIOS_MOD_DIR="$d"
        break
    fi
done

if [ -z "$BIOS_MOD_DIR" ]; then
    echo "WARNING: GRUB i386-pc modules not found. BIOS boot will not work."
    echo "  Install grub-i386-pc (Void) or grub-pc-bin (Debian/Ubuntu)"
else
    # Build list of modules to embed. Newer GRUB folds initrd into linux.mod,
    # so only include initrd if the module file actually exists.
    BIOS_MODS="biosdisk iso9660 normal search configfile linux test echo cat part_gpt part_msdos fat ext2"
    [ -f "$BIOS_MOD_DIR/initrd.mod" ] && BIOS_MODS="$BIOS_MODS initrd"

    grub-mkimage \
        -O i386-pc \
        -o "$WORK/core.img" \
        -p "/boot/grub" \
        -d "$BIOS_MOD_DIR" \
        $BIOS_MODS

    cat "$BIOS_MOD_DIR/cdboot.img" "$WORK/core.img" > "$ISO_DIR/boot/grub/i386-pc/eltorito.img"

    cp "$BIOS_MOD_DIR/"*.mod "$ISO_DIR/boot/grub/i386-pc/" 2>/dev/null || true
    cp "$BIOS_MOD_DIR/"*.lst "$ISO_DIR/boot/grub/i386-pc/" 2>/dev/null || true
    echo "  BIOS boot image: OK"
fi

# ============================================================
# GRUB UEFI boot image
# ============================================================
echo "Building GRUB UEFI boot image..."

EFI_MOD_DIR=""
for d in /usr/lib/grub/x86_64-efi /usr/share/grub/x86_64-efi "$ROOTFS/usr/lib/grub/x86_64-efi"; do
    if [ -d "$d" ]; then
        EFI_MOD_DIR="$d"
        break
    fi
done

if [ -z "$EFI_MOD_DIR" ]; then
    echo "WARNING: GRUB x86_64-efi modules not found. UEFI boot will not work."
    echo "  Install grub-x86_64-efi (Void) or grub-efi-amd64-bin (Debian/Ubuntu)"
else
    grub-mkstandalone \
        --format=x86_64-efi \
        --output="$ISO_DIR/EFI/BOOT/BOOTX64.EFI" \
        --locales="" \
        --fonts="" \
        --modules="part_gpt part_msdos fat iso9660 normal search configfile linux test echo all_video" \
        "boot/grub/grub.cfg=$ISO_DIR/boot/grub/grub.cfg"

    # Create EFI System Partition image for ISO, sized to fit BOOTX64.EFI + overhead
    EFI_IMG="$ISO_DIR/efiboot.img"
    EFI_BINARY_KB=$(( $(stat -c%s "$ISO_DIR/EFI/BOOT/BOOTX64.EFI") / 1024 ))
    EFI_SIZE_KB=$(( EFI_BINARY_KB + 1024 ))
    dd if=/dev/zero of="$EFI_IMG" bs=1K count=$EFI_SIZE_KB 2>/dev/null
    mformat -i "$EFI_IMG" -F ::
    mmd -i "$EFI_IMG" ::/EFI ::/EFI/BOOT
    mcopy -i "$EFI_IMG" "$ISO_DIR/EFI/BOOT/BOOTX64.EFI" ::/EFI/BOOT/BOOTX64.EFI
    echo "  UEFI boot image: OK"
fi

# ============================================================
# Assemble ISO with xorriso (two-pass for squash_pread)
# ============================================================
HAS_BIOS=false
HAS_EFI=false
[ -f "$ISO_DIR/boot/grub/i386-pc/eltorito.img" ] && HAS_BIOS=true
[ -f "$ISO_DIR/efiboot.img" ] && HAS_EFI=true

assemble_iso() {
    if $HAS_BIOS && $HAS_EFI && [ -f "$BIOS_MOD_DIR/boot_hybrid.img" ]; then
        xorriso -as mkisofs \
            -o "$ISO_FILE" -V "$ISO_LABEL" -R -J \
            -b boot/grub/i386-pc/eltorito.img -no-emul-boot -boot-load-size 4 \
            -boot-info-table --grub2-boot-info \
            -eltorito-alt-boot -e efiboot.img -no-emul-boot \
            -isohybrid-mbr "$BIOS_MOD_DIR/boot_hybrid.img" -isohybrid-gpt-basdat \
            "$ISO_DIR"
    else
        XORRISO_ARGS="-as mkisofs -o $ISO_FILE -V $ISO_LABEL -R -J"
        $HAS_BIOS && XORRISO_ARGS="$XORRISO_ARGS -b boot/grub/i386-pc/eltorito.img -no-emul-boot -boot-load-size 4 -boot-info-table --grub2-boot-info"
        $HAS_EFI && XORRISO_ARGS="$XORRISO_ARGS -eltorito-alt-boot -e efiboot.img -no-emul-boot"
        XORRISO_ARGS="$XORRISO_ARGS $ISO_DIR"
        eval xorriso $XORRISO_ARGS
    fi
}

rebuild_efi() {
    if [ -z "$EFI_MOD_DIR" ] || [ ! -d "$EFI_MOD_DIR" ]; then
        return 0
    fi
    grub-mkstandalone --format=x86_64-efi \
        --output="$ISO_DIR/EFI/BOOT/BOOTX64.EFI" --locales="" --fonts="" \
        --modules="part_gpt part_msdos fat iso9660 normal search configfile linux test echo all_video" \
        "boot/grub/grub.cfg=$ISO_DIR/boot/grub/grub.cfg"
    EFI_IMG="$ISO_DIR/efiboot.img"
    EFI_SIZE_KB=$(( $(stat -c%s "$ISO_DIR/EFI/BOOT/BOOTX64.EFI") / 1024 + 1024 ))
    dd if=/dev/zero of="$EFI_IMG" bs=1K count=$EFI_SIZE_KB 2>/dev/null
    mformat -i "$EFI_IMG" -F ::
    mmd -i "$EFI_IMG" ::/EFI ::/EFI/BOOT
    mcopy -i "$EFI_IMG" "$ISO_DIR/EFI/BOOT/BOOTX64.EFI" ::/EFI/BOOT/BOOTX64.EFI
}

get_squashfs_lba() {
    xorriso -indev "$ISO_FILE" -find /live -name root.squashfs -exec report_lba -- 2>&1 \
        | grep 'root\.squashfs' | head -1 | awk -F',' '{gsub(/[^0-9]/,"",$2); print $2}'
}

echo "Assembling ISO (pass 1)..."
assemble_iso
echo "  Pass 1: $(du -sh "$ISO_FILE" | cut -f1)"

# ============================================================
# Patch squash_pread: bypass iso9660 VFS for squashfs access
# ============================================================
# iso9660 VFS open() deadlocks on QEMU ATAPI CD-ROM emulation.
# dynamod.squash_pread=LBA:BYTES lets dynamod-init pread the squashfs
# directly from the raw block device, bypassing iso9660 entirely.

echo "Patching squash_pread offset into GRUB config..."
SQUASH_SIZE=$(stat -c%s "$ISO_DIR/live/root.squashfs")
SQUASH_LBA=$(get_squashfs_lba)

if [ -n "$SQUASH_LBA" ] && [ "$SQUASH_LBA" -gt 0 ] 2>/dev/null; then
    echo "  squashfs: LBA=$SQUASH_LBA size=$SQUASH_SIZE bytes"

    sed -i "s|rootwait|dynamod.squash_pread=${SQUASH_LBA}:${SQUASH_SIZE} rootwait|g" \
        "$ISO_DIR/boot/grub/grub.cfg"

    rebuild_efi

    echo "  Rebuilding ISO (pass 2, with squash_pread)..."
    assemble_iso

    VERIFY_LBA=$(get_squashfs_lba)
    if [ "$VERIFY_LBA" != "$SQUASH_LBA" ]; then
        echo "  LBA shifted ($SQUASH_LBA -> $VERIFY_LBA), correcting..."
        sed -i "s|dynamod\.squash_pread=[^ ]*|dynamod.squash_pread=${VERIFY_LBA}:${SQUASH_SIZE}|g" \
            "$ISO_DIR/boot/grub/grub.cfg"
        rebuild_efi
        assemble_iso
        echo "  Corrected to LBA=$VERIFY_LBA"
    else
        echo "  Verified: LBA stable at $SQUASH_LBA"
    fi
else
    echo "  WARNING: could not determine squashfs LBA from ISO"
    echo "  squash_pread bypass not available — live boot may hang on emulated ATAPI."
fi

echo ""
echo "=== ISO Build Complete ==="
echo "  $ISO_FILE ($(du -sh "$ISO_FILE" | cut -f1))"
echo ""
echo "Test with QEMU:"
echo "  make test-qemu          # graphical"
echo "  make test-qemu-serial   # serial console"
echo ""
echo "Write to USB:"
echo "  sudo dd if=$ISO_FILE of=/dev/sdX bs=4M status=progress oflag=sync"
