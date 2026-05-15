# Development shell for building Eclipse Linux (rootfs / ISO) and dynamod
# on NixOS or any system with Nix installed.
#
# Usage:
#   nix-shell                 # enter dev shell
#   make dynamod              # builds dynamod via neomake
#   make installer            # builds the eclipse-installer (Rust + musl)
#   sudo make rootfs          # downloads Void rootfs, chroots, installs packages
#   sudo make iso             # produces build/eclipse-linux-<ver>.iso
#
# rootfs/iso targets shell out to `sudo` for chroot/mount/losetup; Nix can't
# sandbox those. If you'd rather not run those on your host, use
# Dockerfile.build instead: it produces the same ISO in a privileged container.

{ pkgs ? import <nixpkgs> { } }:

let
  # Pinned to current origin/main of sinisterMage/neomake. Update both the
  # `rev` here and the matching rev in .github/workflows/release.yml when
  # bumping.
  neomakeRev = "aaa042162542db06912bdf08cc26be67f8a8ad68";

  neomakeSrc = pkgs.fetchFromGitHub {
    owner = "sinisterMage";
    repo = "neomake";
    rev = neomakeRev;
    hash = "sha256-qXEBu7hWwA4+ugNb02nlFEjXIaC0TNMA02UuQU1zfho=";
  };

  neomake = pkgs.rustPlatform.buildRustPackage {
    pname = "neomake";
    version = "0.1.0-${builtins.substring 0 7 neomakeRev}";
    src = neomakeSrc;

    cargoLock = {
      lockFileContents = builtins.readFile "${neomakeSrc}/Cargo.lock";
    };

    # The workspace has three crates; we only ship the CLI binary.
    cargoBuildFlags = [ "-p" "neomake" ];
    cargoTestFlags = [ "-p" "neomake" ];

    # neomake's test suite spawns subprocesses + touches the filesystem; skip
    # in the dev-shell derivation to keep nix-shell entry fast. Tests still
    # run via `cargo test` upstream.
    doCheck = false;

    meta = with pkgs.lib; {
      description = "TOML/TOMLX-driven parallel build orchestrator with content-addressable caching";
      homepage = "https://github.com/sinisterMage/neomake";
      license = with licenses; [ mit asl20 ];
      mainProgram = "neomake";
    };
  };

in
pkgs.mkShell {
  name = "eclipse-linux-dev";

  packages = with pkgs; [
    # --- Compilers / toolchains ---------------------------------------------
    zig_0_15    # dynamod-init pins Zig 0.15.2 (matches dynamod/docker/Dockerfile)
    rustup      # Rust + cargo; users still run `rustup target add x86_64-unknown-linux-musl` once
    musl        # libc for static linking targets
    pkg-config

    # --- Build orchestration ------------------------------------------------
    neomake     # custom derivation (see let-binding above)

    # --- ISO + rootfs tooling (scripts/build-iso.sh) ------------------------
    squashfsTools   # mksquashfs
    xorriso         # ISO assembly
    grub2           # grub-mkimage + grub-mkstandalone (BIOS + EFI on x86_64)
    mtools          # mformat / mmd / mcopy (EFI FAT image)
    cpio
    gzip
    xz
    gnutar

    # --- Network / download tools (scripts/build-rootfs.sh) -----------------
    wget
    curl
    unzip

    # --- Installer + chroot helpers -----------------------------------------
    dialog          # eclipse-install TUI
    parted          # disk partitioning
    rsync           # rootfs copy to target
    util-linux      # mount, losetup, lsblk, blkid

    # --- Optional: testing --------------------------------------------------
    qemu            # `make test-qemu*`; KVM acceleration depends on the host
  ];

  shellHook = ''
    echo "Eclipse Linux dev shell"
    echo "  zig       $(zig version)"
    echo "  cargo     $(cargo --version 2>/dev/null || echo 'run: rustup default stable')"
    echo "  neomake   $(neomake --version)"
    echo ""
    echo "First-time setup (one-shot):"
    echo "  rustup default stable"
    echo "  rustup target add x86_64-unknown-linux-musl"
    echo ""
    echo "Then:"
    echo "  make dynamod    # build dynamod (zig + rust)"
    echo "  make installer  # build the eclipse-installer"
    echo "  sudo make iso   # produce the bootable ISO (needs root for chroot)"
  '';
}
