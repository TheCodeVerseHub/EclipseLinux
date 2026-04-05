# Eclipse Linux

Eclipse Linux is an experimental Void Linux (musl) based distribution that boots with the **dynamod** init system. This repository contains the build tooling to generate a live ISO (BIOS + UEFI) and a TUI installer.

## Status / scope

- Target architecture: `x86_64`
- Base: Void Linux musl rootfs tarball + packages installed via `xbps` in a chroot
- Output: a hybrid live ISO with SquashFS rootfs and GRUB boot (BIOS + UEFI)

## Repository layout

- `Makefile` — main entrypoints (`make iso`, `make test-qemu`, etc.)
- `scripts/build-rootfs.sh` — builds `build/rootfs` from a Void musl rootfs tarball
- `scripts/build-iso.sh` — turns `build/rootfs` into `build/eclipse-linux-<version>.iso`
- `scripts/eclipse-install` — dialog-based installer that runs inside the live ISO
- `config/` — branding and boot config (`os-release`, `motd`, `issue`, `grub-live.cfg`)

## Prerequisites

### Source dependency: `dynamod/`

The build expects a `dynamod/` directory at the repo root (see `Makefile` and the scripts). It is intentionally ignored by `.gitignore`, so you need to provide it locally (e.g. clone it into `./dynamod`).

### Host build dependencies

You need a Linux host with `sudo` and the ability to `mount`, `chroot`, and create loop devices.

The ISO builder checks for these commands:

- `mksquashfs` (from `squashfs-tools`)
- `xorriso`
- `grub-mkimage`, `grub-mkstandalone`
- `mformat`, `mmd`, `mcopy` (from `mtools`)
- `cpio`, `gzip`

The rootfs builder also needs `tar` with xz support and either `wget` or `curl`.

Package hints (also printed by the scripts):

- Void: `xbps-install -S squashfs-tools xorriso grub grub-x86_64-efi mtools cpio`
- Arch: `pacman -S squashfs-tools xorriso grub mtools cpio`

### Optional: QEMU test dependencies

- `qemu-system-x86_64`
- `qemu-img` (only for `make test-qemu-install`)

## Quick start

1) Build the init system bits (runs unprivileged):

```sh
make dynamod
```

2) Build the root filesystem (invokes `sudo` internally):

```sh
make rootfs
```

3) Build the ISO (invokes `sudo` internally):

```sh
make iso
```

The ISO is written to:

- `build/eclipse-linux-<ECLIPSE_VERSION>.iso`

## Testing in QEMU

After `make iso`:

```sh
make test-qemu          # graphical
make test-qemu-serial   # serial console
make test-qemu-install  # boots with a blank qcow2 disk
```

## Writing to a USB drive

`build-iso.sh` prints a `dd` example. Be careful to choose the correct device:

```sh
sudo dd if=build/eclipse-linux-<version>.iso of=/dev/sdX bs=4M status=progress oflag=sync
```

## Build configuration

Environment variables used by the scripts:

- `ECLIPSE_VERSION` — ISO filename version string (default `0.1.0`)
- `VOID_DATE` — Void rootfs tarball date stamp (default `20250202`)

Examples:

```sh
ECLIPSE_VERSION=0.2.0 make iso
VOID_DATE=20250202 make rootfs
```

## Notes

- `make dynamod` should **not** be run under `sudo` (it builds as the current user).
- `scripts/build-rootfs.sh` removes firmware blobs and most locales to shrink the live ISO; expect Wi‑Fi / some GPUs to be missing firmware in the live environment.

## License

GPL-3.0 (see `LICENSE`).
