# Eclipse Linux

Eclipse Linux is an experimental Void Linux (musl) based distribution that boots with the **dynamod** init system instead of runit or systemd. This repository contains the build tooling to generate a live ISO (BIOS + UEFI) and a TUI installer.

Eclipse is, in large part, a distribution built to run dynamod on real hardware: the ISO exists so that a Zig PID 1 with an Erlang/OTP-style Rust service manager can be booted, installed, and driven by a full Wayland desktop rather than only a test VM. See [The dynamod init system](#the-dynamod-init-system) for how the two projects fit together, and [ROADMAP.md](ROADMAP.md) for where this is going.

## Status / scope

- Target architecture: `x86_64`
- Base: Void Linux musl rootfs tarball + packages installed via `xbps` in a chroot
- Init: [dynamod](https://github.com/sinisterMage/dynamod) (`dynamod-init` as PID 1 in both the initramfs and the real root)
- Desktop: niri (Wayland) with Waybar, fuzzel, mako, alacritty, fish
- Output: a hybrid live ISO with SquashFS rootfs and GRUB boot (BIOS + UEFI)

## Repository layout

- `Makefile` — main entrypoints (`make iso`, `make test-qemu`, etc.)
- `scripts/build-rootfs.sh` — builds `build/rootfs` from a Void musl rootfs tarball, installs the dynamod binaries and service configs, and applies Eclipse branding
- `scripts/build-iso.sh` — turns `build/rootfs` into `build/eclipse-linux-<version>.iso`, including the `dynamod-init` initramfs
- `scripts/eclipse-install` — dialog-based installer that runs inside the live ISO (shell fallback)
- `eclipse-installer/` — Rust/ratatui TUI installer, the preferred installer shipped on the ISO
- `config/` — branding and boot config (`os-release`, `motd`, `issue`, `grub-live.cfg`) plus `config/skel/` desktop dotfiles
- `dynamod/` — **not tracked here.** The init system source, cloned in locally (see [Prerequisites](#source-dependency-dynamod))

## The dynamod init system

[dynamod](https://github.com/sinisterMage/dynamod) is the init system Eclipse boots with. It is a two-language design: a deliberately tiny Zig PID 1 that cannot afford to crash, and a Rust service manager that does the interesting work.

| Component | Language | Role |
|-----------|----------|------|
| `dynamod-init` | Zig | PID 1. Mounts pseudo-filesystems, performs the initramfs → real-root transition (disk or live/squashfs/overlay), reaps zombies, supervises `dynamod-svmgr` with exponential backoff, drives shutdown. No heap allocation after init. |
| `dynamod-svmgr` | Rust | The service manager. Parses TOML units, builds a dependency DAG, starts services in parallel via a dynamic frontier algorithm, supervises them with OTP-style restart strategies, applies cgroup v2 limits and namespaces. |
| `dynamodctl` | Rust | Operator CLI, talks to `/run/dynamod/control.sock`. |
| `dynamod-logd` | Rust | Log collection into `/var/log/dynamod/`. |
| `dynamod-logind` | Rust | `org.freedesktop.login1` — sessions, seats, and `TakeDevice` for Wayland GPU access. |
| `dynamod-sd1bridge` | Rust | `org.freedesktop.systemd1` — makes `systemctl`-shaped tooling work. |
| `dynamod-hostnamed` | Rust | `hostname1` / `timedate1` / `locale1`. |

The last three are the "systemd-mimic" layer: clean-room D-Bus services that exist so unmodified desktop software keeps working on a system with no systemd.

### Where dynamod lands in the rootfs

`scripts/build-rootfs.sh` installs the build output into the Void rootfs:

| Source | Installed path |
|--------|----------------|
| `dynamod/zig/zig-out/bin/dynamod-init` | `/sbin/dynamod-init` |
| `dynamod/rust/target/**/release/dynamod-svmgr` | `/usr/lib/dynamod/dynamod-svmgr` |
| `…/dynamod-logd` | `/usr/lib/dynamod/dynamod-logd` |
| `…/dynamodctl` | `/usr/bin/dynamodctl` |
| `…/dynamod-logind`, `dynamod-sd1bridge`, `dynamod-hostnamed` | `/usr/lib/dynamod/` (installed only if the binary was built) |
| `dynamod/config/supervisors/*.toml` | `/etc/dynamod/supervisors/` |
| selected `dynamod/config/services/*.toml` | `/etc/dynamod/services/` |
| `dynamod/config/dbus-1/*.conf` | `/usr/share/dbus-1/system.d/` |

`scripts/build-iso.sh` additionally copies `dynamod-init` into the live initramfs, so the same binary is PID 1 both before and after `switch_root`.

Note that dynamod is installed by file copy, not as an `xbps` package — an installed Eclipse system therefore has no upgrade path for the init system. Closing that is tracked in the [roadmap](ROADMAP.md).

### How Eclipse boots it

**Live ISO** (`config/grub-live.cfg`):

```
linux /boot/vmlinuz rdinit=/sbin/dynamod-init dynamod.live=1 \
      dynamod.media=/dev/sr0 dynamod.squashfs=/live/root.squashfs rootwait …
```

`dynamod-init` resolves the media, parses ISO 9660 directly from the raw block device, `pread`s the squashfs into tmpfs, loop-mounts it, stacks an overlayfs, and `switch_root`s. Parsing the ISO in-init rather than mounting it is deliberate: `iso9660` VFS opens can block indefinitely on QEMU's emulated ATAPI CD-ROM. `scripts/build-iso.sh` supports this by locating the squashfs extent after the first `xorriso` pass and patching `dynamod.squash_pread=<LBA>:<BYTES>` into the GRUB config, then reassembling and re-verifying that the LBA did not shift.

**Installed system** (generated by the installer):

```
linux /boot/vmlinuz-<kver> root=UUID=<uuid> rootfstype=<fs> rootwait \
      rdinit=/sbin/dynamod-init init=/sbin/dynamod-init quiet
```

Both `rdinit=` and `init=` are set so dynamod is PID 1 in the initramfs and again after the pivot. The installer writes three entries — normal, `console=ttyS0,115200`, and `single` for recovery.

### Service layout Eclipse ships

Three supervisors come from dynamod unchanged:

- `root` — one-for-one, 20 restarts / 600s
- `early-boot` — one-for-all, 3 / 30s (a failure here takes the whole early-boot group down together)
- `desktop` — one-for-one, 10 / 120s

Eclipse takes `fsck`, `machine-id`, `fstab-mount`, `modules-load`, `bootmisc`, `hostname`, `network`, `sysctl`, `dynamod-logd`, `udev`, `udev-coldplug`, `dynamod-logind`, `dynamod-sd1bridge`, and `dynamod-hostnamed` as-is, then writes its own units in `build-rootfs.sh`:

| Unit | Why Eclipse overrides or adds it |
|------|----------------------------------|
| `remount-root-rw` | override — on the live ISO `/` is overlayfs, where `mount -o remount,rw /` fails with exit 32 even though the upperdir is writable. The Eclipse version treats an `overlay` fstype as success, which unblocks `machine-id`, dbus, and the niri dependency chain. |
| `dbus` | override — creates `/run/dbus` before exec'ing `dbus-daemon --system --nofork`. |
| `agetty-tty1` | override — uses Void's `agetty` from util-linux and orders after `udev-coldplug`. |
| `agetty-ttyS0` | added — serial console getty, which is what `make test-qemu-serial` talks to. |
| `NetworkManager` | added — desktop networking (Wi-Fi, VPN), after `dbus` and `udev-coldplug`. |
| `seatd` | added — libseat provider for niri. Runs with `SEATD_VTBOUND=0`, because VT-bound seat activation never completes under QEMU/virtio and libseat then hangs after niri loads its config. |

Eclipse uses `seatd` rather than `dynamod-logind` as niri's libseat backend (`LIBSEAT_BACKEND=seatd` is exported from `/etc/profile.d/eclipse-wayland.sh`); `dynamod-logind` is still installed and still owns `login1` on the bus.

### Driving it on a running system

```sh
dynamodctl list              # every service and its state
dynamodctl tree              # the supervisor tree
dynamodctl status seatd      # details for one service
dynamodctl restart NetworkManager
dynamodctl shutdown reboot   # poweroff | reboot | halt
```

Logs land in `/var/log/dynamod/`. If the service manager enters a crash loop, PID 1 drops to an interactive shell on `/dev/console`; you can request that explicitly with `dynamod.emergency=1` on the kernel cmdline or `kill -USR2 1`.

To add a service, drop a TOML file into `/etc/dynamod/services/` (or, to make it part of the image, add it to `scripts/build-rootfs.sh`). The full field reference lives in `dynamod/docs/configuration.md`; `dynamod/docs/architecture.md` covers the boot sequence, IPC protocol, and supervisor semantics in depth.

## Prerequisites

### Source dependency: `dynamod/`

The build expects a `dynamod/` directory at the repo root (see `Makefile` and the scripts). It is intentionally ignored by `.gitignore` and is **not** a git submodule, so you need to provide it locally:

```sh
git clone https://github.com/sinisterMage/dynamod.git dynamod
```

Building it needs Zig 0.15.2, a Rust toolchain with the `x86_64-unknown-linux-musl` target, and [`neomake`](https://github.com/sinisterMage/neomake) — `make dynamod` shells out to `neomake run all`. `shell.nix`, `Dockerfile.build`, and `.github/workflows/release.yml` all pin the same neomake revision; if you bump one, bump all three.

Because the clone is unpinned, two checkouts of Eclipse at the same commit can produce different ISOs. Pinning dynamod is a tracked roadmap item.

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

1) Build dynamod — `dynamod-init` plus the Rust binaries (runs unprivileged):

```sh
make dynamod
```

2) Build the TUI installer (runs unprivileged):

```sh
make installer
```

`make rootfs` does **not** depend on this target. If the musl binary is missing, `build-rootfs.sh` silently falls back to installing the older `scripts/eclipse-install` shell installer instead, so build it explicitly before the rootfs.

3) Build the root filesystem (invokes `sudo` internally):

```sh
make rootfs
```

4) Build the ISO (invokes `sudo` internally):

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
- `ECLIPSE_STRIP_FIRMWARE` — set to `1` to drop `/usr/lib/firmware` for a much smaller ISO. Default `0` (firmware kept, so KMS and niri work on real hardware).

Examples:

```sh
ECLIPSE_VERSION=0.2.0 make iso
VOID_DATE=20250202 make rootfs
ECLIPSE_STRIP_FIRMWARE=1 make rootfs   # VM-only / console-oriented image
```

## Notes

- `make dynamod` should **not** be run under `sudo` (it builds as the current user).
- `scripts/build-rootfs.sh` strips most locales and all man/info pages to shrink the live ISO. Firmware is kept by default; set `ECLIPSE_STRIP_FIRMWARE=1` to remove it, which will break Wi‑Fi and many GPUs.
- The live environment has an **empty root password** by design. Don't leave a live session exposed on an untrusted network.

## License

GPL-3.0 (see `LICENSE`).
