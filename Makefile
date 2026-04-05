.PHONY: all dynamod rootfs iso clean distclean test-qemu test-qemu-serial test-qemu-install

ECLIPSE_VERSION ?= 0.1.0
BUILD_DIR       := $(CURDIR)/build
ROOTFS_DIR      := $(BUILD_DIR)/rootfs
ISO_FILE        := $(BUILD_DIR)/eclipse-linux-$(ECLIPSE_VERSION).iso

DYNAMOD_DIR     := $(CURDIR)/dynamod
DYNAMOD_ZIG_OUT := $(DYNAMOD_DIR)/zig/zig-out/bin
DYNAMOD_RUST_OUT := $(shell if [ -d "$(DYNAMOD_DIR)/rust/target/x86_64-unknown-linux-musl/release" ]; then \
	echo "$(DYNAMOD_DIR)/rust/target/x86_64-unknown-linux-musl/release"; else \
	echo "$(DYNAMOD_DIR)/rust/target/release"; fi)

all: iso

# dynamod builds as the current user (needs zig + rustup configured for your user).
# Do NOT run this target under sudo.
dynamod:
	$(MAKE) -C $(DYNAMOD_DIR)

# rootfs and iso require root for chroot/mount/losetup.
# They invoke sudo internally so the top-level `make` can run unprivileged.
rootfs: dynamod
	sudo scripts/build-rootfs.sh $(BUILD_DIR)

iso: rootfs
	sudo scripts/build-iso.sh $(BUILD_DIR)

test-qemu:
	@[ -f $(ISO_FILE) ] || { echo "ERROR: $(ISO_FILE) not found. Run 'make iso' first."; exit 1; }
	@echo "Booting Eclipse Linux ISO in QEMU..."
	@QEMU_EXTRA=""; \
	if [ -w /dev/kvm ]; then \
		QEMU_EXTRA="-enable-kvm -cpu host"; \
		echo "KVM: enabled"; \
	else \
		echo "KVM: not available (will be slower)"; \
	fi; \
	qemu-system-x86_64 \
		$$QEMU_EXTRA \
		-cdrom $(ISO_FILE) \
		-boot d \
		-m 2048M \
		-smp 2 \
		-device virtio-vga-gl \
		-display gtk,gl=on

test-qemu-serial:
	@[ -f $(ISO_FILE) ] || { echo "ERROR: $(ISO_FILE) not found. Run 'make iso' first."; exit 1; }
	@echo "Booting Eclipse Linux ISO in QEMU (serial console)..."
	@QEMU_EXTRA=""; \
	if [ -w /dev/kvm ]; then \
		QEMU_EXTRA="-enable-kvm -cpu host"; \
		echo "KVM: enabled"; \
	else \
		echo "KVM: not available (will be slower)"; \
	fi; \
	qemu-system-x86_64 \
		$$QEMU_EXTRA \
		-cdrom $(ISO_FILE) \
		-boot d \
		-m 2048M \
		-smp 2 \
		-nographic \
		-no-reboot

test-qemu-install:
	@[ -f $(ISO_FILE) ] || { echo "ERROR: $(ISO_FILE) not found. Run 'make iso' first."; exit 1; }
	@echo "Booting Eclipse Linux ISO in QEMU with a blank 20GB disk for installation..."
	@mkdir -p $(BUILD_DIR)
	@[ -f $(BUILD_DIR)/test-disk.qcow2 ] || qemu-img create -f qcow2 $(BUILD_DIR)/test-disk.qcow2 20G
	@QEMU_EXTRA=""; \
	if [ -w /dev/kvm ]; then \
		QEMU_EXTRA="-enable-kvm -cpu host"; \
		echo "KVM: enabled"; \
	else \
		echo "KVM: not available (will be slower)"; \
	fi; \
	qemu-system-x86_64 \
		$$QEMU_EXTRA \
		-cdrom $(ISO_FILE) \
		-drive file=$(BUILD_DIR)/test-disk.qcow2,format=qcow2,if=virtio \
		-boot d \
		-m 2048M \
		-smp 2 \
		-vga virtio \
		-display gtk,gl=on

clean:
	rm -rf $(BUILD_DIR)

distclean: clean
	$(MAKE) -C $(DYNAMOD_DIR) clean
