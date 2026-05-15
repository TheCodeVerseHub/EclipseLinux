# Runtime Eclipse Linux container — dynamod-init as PID 1, on a Void-musl
# base with the container-appropriate subset of Eclipse's package set.
#
# Prerequisite: this build expects `dynamod/` to be present alongside the
# Dockerfile (clone it from https://github.com/sinisterMage/dynamod). The
# directory is .gitignored upstream and not vendored.
#
# Build:
#   git clone https://github.com/sinisterMage/dynamod.git dynamod
#   docker build -t eclipse-linux .
#
# Run (privileged because dynamod-init mounts /proc, /sys, /dev itself —
# same requirement as dynamod/docker/docker-compose.yml):
#   docker run --rm -it --privileged --tty eclipse-linux
#
#   # Inside the container:
#   #   dynamodctl tree     # show supervised service tree
#   #   dynamodctl status   # show running services

# ----------------------------------------------------------------------------
# Stage 1: build dynamod (zig PID-1 + rust service manager / tools)
# ----------------------------------------------------------------------------
FROM ubuntu:24.04 AS builder

ARG ZIG_VERSION=0.15.2
ARG NEOMAKE_REV=aaa042162542db06912bdf08cc26be67f8a8ad68
ARG RUST_TARGET=x86_64-unknown-linux-musl

ENV DEBIAN_FRONTEND=noninteractive \
    PATH=/root/.cargo/bin:/usr/local/bin:/usr/bin:/bin

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl xz-utils musl-tools build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL "https://ziglang.org/download/${ZIG_VERSION}/zig-x86_64-linux-${ZIG_VERSION}.tar.xz" \
        | tar -xJ -C /opt \
    && ln -s "/opt/zig-x86_64-linux-${ZIG_VERSION}/zig" /usr/local/bin/zig

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain stable --profile minimal \
    && rustup target add "${RUST_TARGET}"

# neomake — pinned to the same revision as shell.nix, Dockerfile.build, and CI.
RUN cargo install --git https://github.com/sinisterMage/neomake \
        --rev "${NEOMAKE_REV}" \
        --bin neomake

# Only the dynamod tree is needed for the build; the .dockerignore keeps the
# rest of the Eclipse repo out of the build context cache invalidation path.
COPY dynamod /src/dynamod
WORKDIR /src/dynamod
RUN neomake run all

# ----------------------------------------------------------------------------
# Stage 2: Void-musl runtime
# ----------------------------------------------------------------------------
FROM ghcr.io/void-linux/void-musl:latest AS runtime

# Container-appropriate subset of scripts/build-rootfs.sh's package list:
# drop kernel, firmware, GRUB, niri / Wayland, seatd, mesa, Waybar, mako,
# polkit, pipewire, wireplumber, fonts, cursor theme — none of those have a
# job inside a container PID-1 environment. Keep dbus (for the dynamod
# systemd-mimic D-Bus layer), the shell + dev userland, and Python.
RUN xbps-install -Syu xbps \
    && xbps-install -Sy \
        dbus \
        util-linux \
        busybox \
        ca-certificates \
        fish-shell \
        starship \
        tmux \
        lsd \
        zoxide \
        jq \
        python3 \
        bottom \
        yazi \
        iproute2 \
        procps-ng \
    && rm -rf /var/cache/xbps

# dynamod binaries — same destinations as scripts/build-rootfs.sh installs
# them onto the live rootfs.
COPY --from=builder /src/dynamod/zig/zig-out/bin/dynamod-init                                              /sbin/dynamod-init
COPY --from=builder /src/dynamod/rust/target/x86_64-unknown-linux-musl/release/dynamod-svmgr               /usr/lib/dynamod/dynamod-svmgr
COPY --from=builder /src/dynamod/rust/target/x86_64-unknown-linux-musl/release/dynamodctl                  /usr/bin/dynamodctl
COPY --from=builder /src/dynamod/rust/target/x86_64-unknown-linux-musl/release/dynamod-logd                /usr/lib/dynamod/dynamod-logd
COPY --from=builder /src/dynamod/rust/target/x86_64-unknown-linux-musl/release/dynamod-logind              /usr/lib/dynamod/dynamod-logind
COPY --from=builder /src/dynamod/rust/target/x86_64-unknown-linux-musl/release/dynamod-sd1bridge           /usr/lib/dynamod/dynamod-sd1bridge
COPY --from=builder /src/dynamod/rust/target/x86_64-unknown-linux-musl/release/dynamod-hostnamed           /usr/lib/dynamod/dynamod-hostnamed

# Supervisor + container-tuned service overlay from dynamod (reused verbatim
# — it's already shaped for a containerized dynamod PID 1).
COPY dynamod/config/supervisors/root.toml       /etc/dynamod/supervisors/root.toml
COPY dynamod/config/supervisors/early-boot.toml /etc/dynamod/supervisors/early-boot.toml
COPY dynamod/config/supervisors/desktop.toml   /etc/dynamod/supervisors/desktop.toml
COPY dynamod/docker/services/                   /etc/dynamod/services/

# The dynamod/docker/services/ overlay was shaped for Alpine's
# busybox-everything layout. Drop two services that don't fit Void:
#   * agetty-tty1   — expects /sbin/getty (Alpine busybox) and a real tty1
#                     device; neither exists in a container.
#   * syslog        — expects busybox-style `syslogd -n -O <file>` flags
#                     that Void's syslog packages don't share. dynamod-logd
#                     already captures all service stdout/stderr, so a
#                     separate syslogd is redundant in a container anyway.
# /sbin/sysctl (procps-ng) resolves automatically via Void's merged /usr.
RUN rm -f /etc/dynamod/services/agetty-tty1.toml \
         /etc/dynamod/services/syslog.toml

# Eclipse branding
COPY config/os-release /etc/os-release
COPY config/motd       /etc/motd

RUN mkdir -p /var/lib/dynamod /var/log/dynamod /run/dynamod \
    && echo "eclipse-linux" > /etc/hostname

ENTRYPOINT ["/sbin/dynamod-init"]
