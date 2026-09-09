# Eclipse Linux Roadmap

This roadmap is derived from a gap analysis of the repository at commit `67e3d4f`. Every item below traces to something concretely missing or broken in the tree today — the gap inventory records the evidence, and the milestones turn those gaps into ordered work.

The milestone work items are tracked on the [Eclipse Linux Roadmap project board](https://github.com/orgs/TheCodeVerseHub/projects/6), where each card carries its phase and the gap IDs it closes. This file remains the source of truth; the board is a view of it.

Eclipse currently does one thing well: it produces a bootable hybrid ISO that runs a Wayland desktop on a non-systemd, non-runit init. What it does not yet have is a build you can reproduce, a test that tells you when you broke it, or a story for what happens to an installed system after install day. That ordering — reproducible, then verifiable, then hardened, then maintainable — is the shape of this roadmap.

## How to read this

- **Milestones** are ordered by dependency, not by calendar. M1 and M2 unblock everything else: until the build is pinned and CI exists, every later change is unverifiable.
- **Severity** in the inventory is about user impact: *high* means it burns a user or a contributor today, *medium* means it will, *low* means it is untidy.
- Items marked **↑upstream** belong in [dynamod](https://github.com/sinisterMage/dynamod), not here, and are listed only because Eclipse is blocked on them.

---

## Gap inventory

### Build integrity

| ID | Gap | Evidence | Severity |
|----|-----|----------|----------|
| B1 | `dynamod/` is cloned unpinned at build time, so the same Eclipse commit produces different ISOs on different days | `.gitignore:5`, `.github/workflows/release.yml:25` (`git clone --depth 1`, no ref) | High |
| B2 | No download is verified. The Void rootfs tarball, three Nerd Font archives, and the Bibata cursor theme are fetched over the network and used without a checksum or signature check | `scripts/build-rootfs.sh:71-93`, `:205-229` | High |
| B3 | `make rootfs` does not depend on `make installer`. From a clean tree, `make iso` silently ships the older shell installer instead of the Rust one | `Makefile:33`, `scripts/build-rootfs.sh:519-525` | High |
| B4 | `VOID_DATE` defaults to a fixed `20250202` against `live/current`, a directory Void prunes. The default will eventually 404 with no fallback | `scripts/build-rootfs.sh:25-30` | Medium |
| B5 | `ECLIPSE_VERSION` has four independent defaults and `config/os-release` hardcodes `0.1.0`, so `ECLIPSE_VERSION=0.2.0 make iso` yields an ISO named 0.2.0 whose `/etc/os-release` says 0.1.0 | `Makefile:3`, `build-iso.sh:22`, `build-rootfs.sh:32`, `config/os-release:4-5` | Medium |
| B6 | The pinned `neomake` revision is duplicated in three files with only a comment holding them in sync | `shell.nix:21`, `Dockerfile.build:23`, `Dockerfile:24`, `release.yml:89` | Low |
| B7 | Releases publish `SHA256SUMS` but nothing signs it, so the checksum and the artifact come from the same channel | `release.yml:106-149` | Medium |

### Testing and CI

| ID | Gap | Evidence | Severity |
|----|-----|----------|----------|
| C1 | There is no CI on pull requests. `release.yml` is the only workflow and it only runs on tags and manual dispatch | `.github/workflows/` contains one file | High |
| C2 | Zero automated tests exist. `eclipse-installer` is 3,152 lines of Rust — including disk partitioning and password handling — with no `#[test]` anywhere | `grep -rn '#\[test\]' eclipse-installer/src` returns nothing | High |
| C3 | No lint gate. ~1,800 lines of POSIX shell across `scripts/` never see `shellcheck`; the Rust never sees `clippy` or `cargo fmt --check` | no config or workflow references either | Medium |
| C4 | Boot is verified by a human watching QEMU. Nothing automatically asserts that the ISO boots and that services reach ready | `Makefile:39-134` are all interactive targets | High |
| C5 | The installer is never exercised automatically, so partitioning, GRUB install, and initramfs assembly regress silently | no non-interactive install path exists | High |

### Security

| ID | Gap | Evidence | Severity |
|----|-----|----------|----------|
| S1 | The shipped D-Bus system bus policy is fully permissive — `<allow own="*"/>`, `send_destination="*"`, and `eavesdrop="true"` for the default context. Any local user can own any bus name and read all system bus traffic | `scripts/build-rootfs.sh:420-437` | High |
| S2 | `/etc/resolv.conf` is hardcoded to `8.8.8.8` on every installed system, overriding whatever NetworkManager would have configured | `eclipse-installer/src/install.rs:443-445`, `build-rootfs.sh:111-112` | Medium |
| S3 | No Secure Boot support; the ISO cannot boot on stock UEFI firmware with Secure Boot enabled | `build-iso.sh:276-292` uses unsigned `grub-mkstandalone` output | Medium |
| S4 | `config/os-release` points `HOME_URL` and `BUG_REPORT_URL` at `sinisterMage/EclipseLinux`, but the project lives at `TheCodeVerseHub/EclipseLinux`. Every installed system tells its users to file bugs at the wrong repo | `config/os-release:6-7` vs `git remote` and `SECURITY.md:8` | High |
| S5 | The installer permits a passwordless root **and** a passwordless user on the installed system, with no warning at the confirmation step | `install.rs:502-507` | Medium |
| S6 | Passwords are held as plaintext `String` in `InstallConfig`, cloned into the install thread, and never zeroized | `config.rs:100-103` | Low |

### Installer

| ID | Gap | Evidence | Severity |
|----|-----|----------|----------|
| I1 | Two installers implement the same eleven stages and have already drifted; which one ships depends on whether a build artifact happened to exist | `eclipse-installer/` vs `scripts/eclipse-install` | High |
| I2 | Whole-disk install only. `wipefs -af` then a fresh GPT — no manual partitioning, no install-to-existing-partition, no dual boot | `install.rs:182-240` | High |
| I3 | No swap partition or swapfile is created | `partition_disk()` creates two partitions at most | Medium |
| I4 | No disk encryption (LUKS) option | absent from the wizard | Medium |
| I5 | No keyboard layout or locale selection. Timezone is the only localisation the wizard offers, and `build-rootfs.sh` deletes all non-English locales | `wizard/` has `timezone.rs` but no keymap/locale step; `build-rootfs.sh:620-624` | High |
| I6 | Failure leaves the target mounted and half-written with no rollback and no offer to retain the log off the tmpfs | `do_install()` returns on first error; `LOG_PATH` is `/tmp/eclipse-install.log` | Medium |
| I7 | Partition suffix detection tests for the substring `nvme`, which misidentifies other `p`-suffixed devices (mmcblk, loop, md, dm) | `config.rs:125-131` | Medium |

### Lifecycle and packaging

| ID | Gap | Evidence | Severity |
|----|-----|----------|----------|
| L1 | dynamod is installed by `install -Dm755` file copy, not as a package. An installed Eclipse system has **no upgrade path for its own init system** | `build-rootfs.sh:235-244` | High |
| L2 | No Eclipse package repository. `xbps` on an installed system points only at Void, which does not carry dynamod, the installer, or Eclipse's configs | no `xbps.d` config is written | High |
| L3 | Service unit files are heredocs inside a build script rather than tracked files, so they cannot be diffed, linted, or reviewed as configuration | `build-rootfs.sh:265-437` | Medium |
| L4 | No release process documentation and no changelog; release notes are auto-generated from commit subjects | `release.yml:127` | Medium |

### Desktop and defaults

| ID | Gap | Evidence | Severity |
|----|-----|----------|----------|
| D1 | Audio does not work out of the box. `pipewire` and `wireplumber` are installed but nothing starts them — no dynamod unit, no entry in the niri autostart | `build-rootfs.sh:173-174` vs `config/skel/.config/niri/autostart.sh` | High |
| D2 | `greetd` and `tuigreet` are installed and `/etc/greetd/config.toml` plus a wayland-session desktop file are written, but greetd was abandoned in `8898cae` and no unit ever starts it. Dead weight and a misleading config | `build-rootfs.sh:194`, `:440-462` | Medium |
| D3 | `config/skel/` ships another person's dotfiles: tmux session scripts named `Masters-Thesis.sh`, `DawnCraft-Server.sh`, `AgriApp.sh`, `Advance-Guidance-AID.sh`, `Rust-freelist.sh`, `nvim-nordic.sh`. Every Eclipse user gets them | `config/skel/.config/tmux/sessions/` | High |
| D4 | `config/skel/.config/fish/fish_variables` is tracked machine state (`__fish_initialized`, `fish_greeting_shown`) rather than configuration | `config/skel/.config/fish/fish_variables` | Low |
| D5 | `nm-applet` is installed but never started, so Waybar's network module has no tray companion | `build-rootfs.sh:191` vs `autostart.sh` | Low |
| D6 | `wallpaper-test.jpg` (223 KB) sits at the repo root, referenced by nothing but `.dockerignore` | repo root | Low |
| D7 | wlogout icons are installed twice — into `/usr/share/eclipse/wlogout/icons/` and again via `skel` into every home directory | `build-rootfs.sh:566-568` | Low |
| D8 | No offline/first-boot experience: no welcome app, no post-install checklist, no way to discover `dynamodctl` other than reading this repo | — | Medium |

### Project and governance

| ID | Gap | Evidence | Severity |
|----|-----|----------|----------|
| P1 | No `CODE_OF_CONDUCT.md`, though issue templates and CODEOWNERS assume an organisation with maintainer and contributor teams | `.github/` | Low |
| P2 | `CODEOWNERS` lists each team twice on the doc and security lines (`@TheCodeVerseHub/documentation @TheCodeVerseHub/documentation`), which is harmless but suggests the file was never verified against real review assignment | `.github/CODEOWNERS:11-15` | Low |
| P3 | Only `x86_64` is supported. No aarch64 path exists, in the scripts or the roadmap-to-date | `build-rootfs.sh:26`, `build-iso.sh` | Low |

---

## Milestones

### M1 — Trustworthy builds

*Goal: two people building the same commit get the same ISO, and can tell whether what they downloaded is what was published.*

Addresses **B1–B7**, **S4**.

1. Pin dynamod. Either add it as a git submodule or introduce a `DYNAMOD_REF` variable consumed by the Makefile, CI, and both Dockerfiles. The chosen ref must be printed at build time and recorded in the ISO (e.g. `/etc/eclipse-build-info`).
2. Verify every download. Ship an expected `sha256` for the Void tarball, the Nerd Font archives, and the Bibata theme; fail the build on mismatch. Verify the Void tarball's signature where available.
3. Make `rootfs` depend on `installer`, and turn the shell-installer fallback into an explicit opt-in rather than a silent substitution.
4. Resolve `VOID_DATE` at build time against the mirror index, with the pinned date as a preferred value and a clear error naming the available dates when it is gone.
5. Collapse the version defaults to one source. Generate `os-release` from `ECLIPSE_VERSION` instead of tracking a file with the version baked in.
6. Single-source the neomake pin (a `.neomake-rev` file read by all four consumers).
7. Sign `SHA256SUMS` in the release workflow and document verification in the release notes.
8. Point `config/os-release`'s `HOME_URL` and `BUG_REPORT_URL` at `TheCodeVerseHub/EclipseLinux`, so installed systems send users to the repo that actually exists.

**Exit criteria:** two clean builds of the same commit on different hosts produce byte-identical squashfs input, the build fails loudly on a tampered download, and a released ISO can be verified end to end from a signature.

### M2 — A test suite that can fail

*Goal: a pull request that breaks the boot cannot be merged without someone being told.*

Addresses **C1–C5**.

1. Add a `ci.yml` running on pull requests: `shellcheck` over `scripts/`, `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo test` for `eclipse-installer`.
2. Refactor the pure logic out of `install.rs` — partition path derivation, fstab generation, grub.cfg rendering, `is_shadow_locked`, kernel version detection — and unit-test it. This is where **I7** gets fixed with a test that covers `nvme`, `mmcblk`, `loop`, `md`, and `sd`/`vd`.
3. Build a headless boot smoke test: boot the ISO with `-nographic`, drive the serial console, and assert that `dynamodctl tree` reports the expected services ready and nothing blocked. Fail on timeout.
4. Add a non-interactive install mode to the installer (config from a file or flags) so an install to a scratch qcow2 can run in CI, followed by a boot of the installed disk.
5. Run the boot smoke test nightly against a fresh dynamod clone, so upstream drift surfaces as a scheduled failure rather than a release-day surprise.

**Exit criteria:** CI runs on every PR; a deliberately broken service unit or a broken partition path fails the build.

### M3 — Security baseline

*Goal: the default install is not more permissive than it needs to be, and the differences from a conventional distro are deliberate and documented.*

Addresses **S1–S3**, **S5**, **S6**, **B7**.

1. Replace the permissive `/etc/dbus-1/system.conf` with a default-deny policy plus explicit per-interface `system.d` rules for `login1`, `systemd1`, `hostname1`, `timedate1`, and `locale1`. dynamod already ships the per-interface policy files — the blanket override is what has to go. Verify with a D-Bus smoke test that the desktop still comes up.
2. Stop writing a hardcoded `resolv.conf`. Let NetworkManager own it, with a symlink or a fallback only when NetworkManager is absent.
3. Warn explicitly in the confirmation step when root or the user account will have no password, and require an extra confirmation.
4. Zeroize password material after use (`zeroize` crate) and keep it out of any log path.
5. Investigate Secure Boot: shim + signed GRUB, or an unsigned-but-documented position. Decide and write it down either way.
6. Document the live environment's empty root password and its intended threat model.

**Exit criteria:** a fresh install passes a basic hardening review — no wildcard bus policy, no surprise DNS server, no silently passwordless root.

### M4 — One installer, and a better one

*Goal: installation stops being the riskiest part of the project.*

Addresses **I1–I7**, **D8**.

1. Retire the duplicate. Make `eclipse-installer` the only implementation and delete `scripts/eclipse-install`, or reduce the shell script to a recovery-only minimal path with that role stated in its header.
2. Add partitioning modes: automatic whole-disk (today's behaviour), install-to-existing-partition, and manual. Dual boot depends on this.
3. Add swap (partition or file, sized from RAM) with an `fstab` entry.
4. Add optional LUKS2 full-disk encryption, including the initramfs and GRUB changes it implies — this needs a dynamod-side unlock story, so scope it with upstream first (**↑upstream**).
5. Add keyboard layout and locale selection, and stop unconditionally deleting non-English locales from the installed system. Keep the aggressive strip for the live ISO only, or generate the selected locale during install.
6. On failure: unmount cleanly, copy the log somewhere persistent, and offer retry or exit rather than dropping the user into a half-installed disk.
7. Add a first-boot welcome step that points at `dynamodctl`, the log locations, and the installed docs.

**Exit criteria:** a dual-boot install onto an existing partition, with swap and a non-US keymap, completes and boots.

### M5 — Lifecycle: an installed system that can be maintained

*Goal: an Eclipse install can receive updates, including to its init system.*

Addresses **L1–L4**, **B5**.

1. Package dynamod as an `xbps` package. `dynamod/dist/void/template` already exists as a starting point; the work is producing a repository-quality template and building it in CI.
2. Package `eclipse-installer` and the Eclipse configs (branding, service units, skel) the same way.
3. Stand up an Eclipse `xbps` repository and write `/etc/xbps.d/` config into the rootfs so installed systems can see it. Sign the repository.
4. Move the heredoc service units out of `build-rootfs.sh` into tracked files under `config/dynamod/services/`, installed by copy. This makes **L3** reviewable and is a prerequisite for packaging them.
5. Provide `eclipse-upgrade` (or document the `xbps-install -Su` path) including what happens when PID 1 itself is replaced on a running system.
6. Adopt a changelog and a written release process.

**Exit criteria:** `xbps-install -Su` on an installed Eclipse system upgrades dynamod and reboots into the new init.

### M6 — Defaults worth shipping

*Goal: the out-of-box desktop works and contains nothing personal to any one contributor.*

Addresses **D1–D7**.

1. Start pipewire and wireplumber — as a user session in the niri autostart, or as dynamod units if a system-wide session is preferred. Audio not working is the most visible defect in the current image.
2. Remove greetd, tuigreet, `/etc/greetd/config.toml`, and the wayland-session desktop entry, or reinstate greetd properly with a unit. Do not ship both a config and the comment explaining why it is unused.
3. Purge the personal tmux session scripts from `config/skel/` and replace them with one neutral example, or drop the directory.
4. Stop tracking `fish_variables`; generate it or let fish create it.
5. Start `nm-applet` from the autostart, or drop the package.
6. Delete `wallpaper-test.jpg`.
7. Install wlogout icons once and point the skel config at the system path.

**Exit criteria:** a fresh install plays audio, shows a working tray, and contains no file naming anyone's thesis.

### M7 — Reach

*Goal: Eclipse runs somewhere other than one maintainer's x86_64 test VM.*

Addresses **P3**, **S3**, plus hardware validation.

1. aarch64 support: Void has an aarch64 musl rootfs, the ISO path needs a UEFI-only equivalent, and dynamod needs to be built for the target (**↑upstream** for anything arch-specific in the Zig init).
2. A published hardware compatibility matrix, driven by the boot smoke test where possible and by user reports otherwise.
3. Secure Boot, if M3 concluded it is worth doing.
4. Installation media beyond ISO: a raw disk image for VMs and SBCs.

**Exit criteria:** an aarch64 image boots to a desktop, and the compatibility matrix has entries that did not come from the maintainer.

---

## Sequencing at a glance

```
M1 Trustworthy builds ──┬──> M2 Test suite ──┬──> M4 Installer ──> M7 Reach
                        │                    │
                        └──> M3 Security ────┴──> M5 Lifecycle
                                                   │
                                             M6 Defaults (independent, do anytime)
```

M6 is deliberately off the critical path: every item in it is small, self-contained, and improves the image immediately. It is good first-contribution territory.

## Explicitly out of scope

- Replacing dynamod, or supporting a second init. Running dynamod is the point of the distribution.
- A graphical (GTK/Qt) installer. The TUI is intentional and works over serial.
- A general-purpose package ecosystem. Eclipse consumes Void's; the Eclipse repository (M5) carries only Eclipse's own packages.

## Open questions

- **Where do the service units live long-term?** Tracked in Eclipse (M5.4) is right for overrides, but several of the current overrides — the overlayfs `remount-root-rw` fix in particular — arguably belong upstream in dynamod. That decision affects how much of `build-rootfs.sh` survives.
- **Is `seatd` the permanent answer for niri, or a workaround?** Eclipse uses seatd rather than `dynamod-logind` for libseat while still running logind for `login1`. If logind's `TakeDevice` path is solid, consolidating removes a daemon and a class of VT-activation bugs.
- **What is the release cadence?** Everything in M1 and M5 is easier to answer once it is known whether releases are per-tag, nightly, or rolling.
