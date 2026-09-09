# Contributing

Thanks for contributing to Eclipse Linux.

## What belongs in this repo

This repository primarily owns:

- ISO/rootfs build scripts (`scripts/`)
- Live boot + branding config (`config/`, including the `config/skel/` desktop dotfiles)
- The live installers (`eclipse-installer/` in Rust, `scripts/eclipse-install` as the shell fallback)
- Top-level build targets (`Makefile`) and the container/Nix build environments

The build expects a `dynamod/` source tree to exist locally, but that code is kept separate — see below.

## Working with dynamod

Eclipse does not vendor [dynamod](https://github.com/sinisterMage/dynamod); it clones it in and consumes its build output. Knowing which side of that line a change belongs on saves a lot of time:

| Symptom | Where the fix goes |
|---------|--------------------|
| PID 1 panics, hangs, or mis-handles `switch_root` / shutdown / zombie reaping | dynamod (`zig/src/`) |
| A supervisor restarts wrongly, dependencies resolve wrongly, cgroups/namespaces misbehave | dynamod (`rust/dynamod-svmgr`) |
| `login1` / `systemd1` / `hostname1` D-Bus behaviour is wrong | dynamod (the systemd-mimic crates) |
| A service unit is missing, misordered, or wired to the wrong path **on Eclipse** | here, in `scripts/build-rootfs.sh` |
| The kernel cmdline, initramfs contents, or squashfs/overlay plumbing is wrong | here, in `scripts/build-iso.sh` / `config/grub-live.cfg` |
| The installed system's GRUB entries or `init=`/`rdinit=` flags are wrong | here, in `eclipse-installer/src/install.rs` |

A change that would help every dynamod user belongs upstream, not in an Eclipse-side override.

### Changing service wiring

`scripts/build-rootfs.sh` copies a selected set of unit files out of `dynamod/config/services/` and then writes several Eclipse-specific ones inline as heredocs (`remount-root-rw`, `dbus`, `agetty-tty1`, `agetty-ttyS0`, `NetworkManager`, `seatd`). Each override exists for a documented reason — overlayfs remount semantics, `/run/dbus` creation, Void's `agetty` path, `SEATD_VTBOUND=0` under virtio. If you add or change one:

- **Say why in a comment.** An override with no rationale is indistinguishable from drift against upstream, and the next person will delete it.
- Prefer adding a new unit over forking an upstream one. If you find yourself editing a copied unit, consider whether the change belongs in dynamod instead.
- Adding a unit to the copy list is not enough — it must also name a supervisor that exists (`root`, `early-boot`, or `desktop`) and its `exec` path has to match where `build-rootfs.sh` installs the binary (`/usr/lib/dynamod/` for daemons, `/usr/bin/` for `dynamodctl`).
- Remember `early-boot` is **one-for-all**: a new unit that fails there takes the whole early-boot group down with it. Put anything non-essential on `root` or `desktop`.

The field reference for unit files is `dynamod/docs/configuration.md`; boot sequence and supervisor semantics are in `dynamod/docs/architecture.md`.

### Debugging a boot failure

Boot with the serial console — it captures everything PID 1 prints before a display is up:

```sh
make test-qemu-serial          # live ISO
make test-qemu-disk-serial     # installed disk
```

Once you have a shell:

```sh
dynamodctl tree                # what started, what is blocked
dynamodctl status <service>    # exit codes and restart counts
cat /var/log/dynamod/*         # service logs
```

If the service manager crash-loops, PID 1 drops to a shell on `/dev/console`. You can force that with `dynamod.emergency=1` on the kernel cmdline, or `kill -USR2 1` from a running system. For live-boot problems specifically, the `Eclipse Linux (live, verbose)` GRUB entry adds `earlyprintk=ttyS0`, which surfaces failures that happen before init gets going.

## Getting set up

1) Ensure you have the host dependencies listed in [README.md](README.md).

2) Provide a `dynamod/` directory at the repo root:

```sh
git clone https://github.com/sinisterMage/dynamod.git dynamod
```

It is `.gitignore`d and not a submodule, so it is your responsibility to keep it current. When you report a build or boot bug, include the dynamod commit you built against — Eclipse does not pin it, so "same Eclipse commit" does not mean "same ISO".

Alternatively, `nix-shell` (see `shell.nix`) or `docker compose run --rm build` (see `Dockerfile.build`) gives you a host with Zig, Rust, and the pinned `neomake` already in place.

3) Verify you can build end-to-end:

```sh
make dynamod
make installer
make rootfs
make iso
```

## Making changes safely

### Scripts

- All scripts are POSIX `sh` (`#!/bin/sh`). Avoid bashisms unless you also switch the shebang and test accordingly.
- Prefer failing fast (`set -e`) and printing actionable error messages.
- If you add new required tools, update the dependency checks in the relevant script and update [README.md](README.md).

### Live rootfs changes

Changes in `scripts/build-rootfs.sh` affect both:

- the live ISO boot, and
- the installed system created by `eclipse-install` (it copies the live rootfs to disk).

When changing packages or init/service wiring, validate:

- `make rootfs` completes
- the ISO boots (`make test-qemu`)
- the installer can complete an installation (`make test-qemu-install`)

### Installer changes

There are currently **two** installers: `eclipse-installer/` (Rust, what the ISO ships when it has been built) and `scripts/eclipse-install` (shell, the fallback `build-rootfs.sh` falls back to). They implement the same eleven-stage install and have already drifted. If you change installation behaviour, change both or explicitly note in the PR which one you left behind.

### Boot configuration

- Live boot menu is in `config/grub-live.cfg`. `scripts/build-iso.sh` rewrites it during the build to inject `dynamod.squash_pread=<LBA>:<BYTES>`, so any edit that changes the `rootwait` token needs to be checked against that `sed`.
- The installed system's GRUB config is generated by `write_grub_cfg()` in `eclipse-installer/src/install.rs` (and mirrored in `scripts/eclipse-install`).

If you change kernel command line flags for live boot, consider whether the installer’s installed entries should also be updated. In particular, both paths must keep `rdinit=/sbin/dynamod-init`, and the installed entries must also keep `init=/sbin/dynamod-init` — dropping either drops dynamod as PID 1 on one side of the pivot.

## Reporting bugs

- Use GitHub Issues for build failures, boot issues, and installer problems.
- Include:
  - host distro + versions of key tools (`grub`, `xorriso`, `mksquashfs`)
  - the exact command you ran (`make iso`, environment variables, etc.)
  - the **dynamod commit** you built against (`git -C dynamod rev-parse HEAD`)
  - relevant logs/output (installer logs go to `/tmp/eclipse-install.log` in the live environment; service logs to `/var/log/dynamod/`)
  - for boot hangs: serial console output from `make test-qemu-serial`, plus `dynamodctl tree` if you reach a shell

## Pull request checklist

- Build still works (`make dynamod && make installer && make iso`)
- QEMU boot test still works (`make test-qemu` or `make test-qemu-serial`)
- If installer-related: validate `make test-qemu-install`, and check both installers (see [Installer changes](#installer-changes))
- If you touched service wiring: `dynamodctl tree` on the booted image shows the expected services ready, with nothing blocked
- Keep changes focused; avoid unrelated formatting-only diffs

## Code style

- Shell: keep it simple, quote variables, and prefer explicit, readable steps over clever one-liners.
- Config files: keep defaults conservative (especially anything that could touch disks/partitioning).

## License

By contributing, you agree your contributions are licensed under the project’s GPL-3.0 license (see `LICENSE`).
