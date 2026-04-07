use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::config::{
    FsType, InstallConfig, BUSYBOX_SYMLINKS, EFI_LABEL, GREETD_SERVICE, GRUB_BIOS_MODULES,
    INITRAMFS_LIB_PATTERNS, ROOT_LABEL, RSYNC_EXCLUDES, TARGET_MNT, USER_GROUPS, USER_SHELL,
};
use crate::log;

#[derive(Debug)]
pub enum Progress {
    Update { percent: u8, message: String },
    Complete,
    Error(String),
}

/// Run a command, capture output, log it, and return an error on non-zero exit.
fn run_cmd(program: &str, args: &[&str]) -> Result<Output, String> {
    log::log(&format!("Running: {} {}", program, args.join(" ")));
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute {}: {}", program, e))?;
    log::log_output(program, &output);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{} failed (exit {}): {}",
            program,
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }
    Ok(output)
}

/// Run a command inside a chroot at TARGET_MNT.
fn run_chroot(program: &str, args: &[&str]) -> Result<Output, String> {
    let mut chroot_args = vec![TARGET_MNT, program];
    chroot_args.extend_from_slice(args);
    run_cmd("chroot", &chroot_args)
}

/// Run a command with stdin piped, inside a chroot.
/// Explicitly drops the stdin handle before waiting so the child sees EOF
/// and doesn't block waiting for more input.
fn run_chroot_stdin(program: &str, args: &[&str], stdin_data: &str) -> Result<Output, String> {
    let mut chroot_args = vec![TARGET_MNT.to_string(), program.to_string()];
    chroot_args.extend(args.iter().map(|s| s.to_string()));

    log::log(&format!(
        "Running (with stdin): chroot {}",
        chroot_args.join(" ")
    ));

    let mut child = Command::new("chroot")
        .args(&chroot_args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn chroot {}: {}", program, e))?;

    // Write stdin data then explicitly drop the handle to close the pipe,
    // ensuring the child receives EOF on stdin before we block on wait.
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| format!("No stdin for chroot {}", program))?;
        stdin
            .write_all(stdin_data.as_bytes())
            .map_err(|e| format!("Failed to write stdin to chroot {}: {}", program, e))?;
        // stdin is dropped here, closing the pipe → child sees EOF
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait for chroot {}: {}", program, e))?;
    log::log_output(&format!("chroot {}", program), &output);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "chroot {} failed (exit {}): {}",
            program,
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }
    Ok(output)
}

fn send(tx: &mpsc::Sender<Progress>, percent: u8, msg: &str) {
    let _ = tx.send(Progress::Update {
        percent,
        message: msg.to_string(),
    });
}

/// Main installation entry point, run in a background thread.
pub fn run_install(config: InstallConfig, tx: mpsc::Sender<Progress>) {
    if let Err(e) = do_install(&config, &tx) {
        log::log(&format!("Installation failed: {}", e));
        let _ = tx.send(Progress::Error(e));
    }
}

fn do_install(config: &InstallConfig, tx: &mpsc::Sender<Progress>) -> Result<(), String> {
    send(tx, 0, "Partitioning disk...");
    log::log("=== Stage 1: Partitioning disk ===");
    partition_disk(config)?;
    log::log("=== Stage 1 complete ===");

    send(tx, 15, "Formatting partitions...");
    log::log("=== Stage 2: Formatting partitions ===");
    format_partitions(config)?;
    log::log("=== Stage 2 complete ===");

    send(tx, 25, "Mounting target filesystem...");
    log::log("=== Stage 3: Mounting target ===");
    mount_target(config)?;
    log::log("=== Stage 3 complete ===");

    send(tx, 30, "Copying system files (this may take several minutes)...");
    log::log("=== Stage 4: Copying rootfs ===");
    copy_rootfs(config, tx)?;
    log::log("=== Stage 4 complete ===");

    send(tx, 70, "Generating fstab...");
    log::log("=== Stage 5: Generating fstab ===");
    generate_fstab(config)?;
    log::log("=== Stage 5 complete ===");

    send(tx, 75, "Configuring system...");
    log::log("=== Stage 6: Configuring system ===");
    configure_system(config)?;
    log::log("=== Stage 6 complete ===");

    send(tx, 80, "Setting up user accounts...");
    log::log("=== Stage 7: Setting up accounts ===");
    setup_accounts(config)?;
    log::log("=== Stage 7 complete ===");

    send(tx, 85, "Installing GRUB bootloader...");
    log::log("=== Stage 8: Installing GRUB ===");
    install_grub(config)?;
    log::log("=== Stage 8 complete ===");

    send(tx, 90, "Writing bootloader configuration...");
    log::log("=== Stage 9: Writing grub.cfg ===");
    let kver = detect_kernel_version()?;
    let root_uuid = get_uuid(&config.root_partition())?;
    write_grub_cfg(config, &kver, &root_uuid)?;
    log::log("=== Stage 9 complete ===");

    send(tx, 95, "Building initramfs...");
    log::log("=== Stage 10: Building initramfs ===");
    build_initramfs(&kver)?;
    log::log("=== Stage 10 complete ===");

    send(tx, 100, "Cleaning up...");
    log::log("=== Stage 11: Cleanup ===");
    cleanup_mounts(config);
    log::log("=== Stage 11 complete ===");

    log::log("Installation completed successfully");
    let _ = tx.send(Progress::Complete);
    Ok(())
}

// ---------------------------------------------------------------------------
// Stage 1: Partition disk
// ---------------------------------------------------------------------------
fn partition_disk(config: &InstallConfig) -> Result<(), String> {
    log::log(&format!("Partitioning {}...", config.disk));
    run_cmd("wipefs", &["-af", &config.disk])?;

    if config.uefi {
        run_cmd(
            "parted",
            &[
                "-s",
                &config.disk,
                "mklabel",
                "gpt",
                "mkpart",
                "ESP",
                "fat32",
                "1MiB",
                "513MiB",
                "set",
                "1",
                "esp",
                "on",
                "mkpart",
                "root",
                config.fs_type.as_str(),
                "513MiB",
                "100%",
            ],
        )?;
    } else {
        run_cmd(
            "parted",
            &[
                "-s",
                &config.disk,
                "mklabel",
                "gpt",
                "mkpart",
                "bios",
                "1MiB",
                "3MiB",
                "set",
                "1",
                "bios_grub",
                "on",
                "mkpart",
                "root",
                config.fs_type.as_str(),
                "3MiB",
                "100%",
            ],
        )?;
    }

    thread::sleep(Duration::from_secs(2));
    let _ = run_cmd("partprobe", &[&config.disk]);
    thread::sleep(Duration::from_secs(1));

    Ok(())
}

// ---------------------------------------------------------------------------
// Stage 2: Format partitions
// ---------------------------------------------------------------------------
fn format_partitions(config: &InstallConfig) -> Result<(), String> {
    log::log("Formatting partitions...");

    if let Some(efi) = config.efi_partition() {
        run_cmd("mkfs.fat", &["-F", "32", "-n", EFI_LABEL, &efi])?;
    }

    let root = config.root_partition();
    match config.fs_type {
        FsType::Ext4 => run_cmd("mkfs.ext4", &["-q", "-L", ROOT_LABEL, &root])?,
        FsType::Btrfs => run_cmd("mkfs.btrfs", &["-f", "-L", ROOT_LABEL, &root])?,
        FsType::Xfs => run_cmd("mkfs.xfs", &["-f", "-L", ROOT_LABEL, &root])?,
    };

    Ok(())
}

// ---------------------------------------------------------------------------
// Stage 3: Mount target
// ---------------------------------------------------------------------------
fn mount_target(config: &InstallConfig) -> Result<(), String> {
    log::log("Mounting target...");

    fs::create_dir_all(TARGET_MNT).map_err(|e| format!("mkdir {}: {}", TARGET_MNT, e))?;

    let root = config.root_partition();
    run_cmd("mount", &[&root, TARGET_MNT])?;

    if let Some(efi) = config.efi_partition() {
        let efi_mnt = format!("{}/boot/efi", TARGET_MNT);
        fs::create_dir_all(&efi_mnt).map_err(|e| format!("mkdir {}: {}", efi_mnt, e))?;
        run_cmd("mount", &[&efi, &efi_mnt])?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Stage 4: Copy rootfs via rsync
// ---------------------------------------------------------------------------
fn copy_rootfs(config: &InstallConfig, tx: &mpsc::Sender<Progress>) -> Result<(), String> {
    log::log(&format!(
        "Copying rootfs from {} to {}...",
        config.squashfs_mount, TARGET_MNT
    ));

    let mut args: Vec<String> = vec![
        "-aHAXx".to_string(),
        "--info=progress2".to_string(),
    ];
    for excl in RSYNC_EXCLUDES {
        args.push(format!("--exclude={}", excl));
    }
    args.push(format!("{}/", config.squashfs_mount));
    args.push(format!("{}/", TARGET_MNT));

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    log::log(&format!("Running: rsync {}", args.join(" ")));
    log::log("rsync: this may take several minutes for a large rootfs...");
    let output = Command::new("rsync")
        .args(&arg_refs)
        .output()
        .map_err(|e| format!("Failed to execute rsync: {}", e))?;
    log::log("rsync: command returned");
    log::log_output("rsync", &output);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("rsync failed: {}", stderr.trim()));
    }

    // Send a 70% update after rsync completes
    send(tx, 70, "File copy complete.");

    // Create required directories and set permissions
    for dir in &["run", "tmp", "proc", "sys", "dev"] {
        let path = format!("{}/{}", TARGET_MNT, dir);
        fs::create_dir_all(&path).map_err(|e| format!("mkdir {}: {}", path, e))?;
    }
    let tmp_path = format!("{}/tmp", TARGET_MNT);
    fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o1777))
        .map_err(|e| format!("chmod {}: {}", tmp_path, e))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Stage 5: Generate fstab
// ---------------------------------------------------------------------------
fn get_uuid(partition: &str) -> Result<String, String> {
    let output = run_cmd("blkid", &["-s", "UUID", "-o", "value", partition])?;
    let uuid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if uuid.is_empty() {
        return Err(format!("Could not determine UUID for {}", partition));
    }
    Ok(uuid)
}

fn generate_fstab(config: &InstallConfig) -> Result<(), String> {
    log::log("Generating fstab...");

    let root_uuid = get_uuid(&config.root_partition())?;
    let fstab_path = format!("{}/etc/fstab", TARGET_MNT);

    let mut content = format!(
        "# Eclipse Linux fstab - generated by eclipse-install\nUUID={}    /         {}    defaults    0 1\n",
        root_uuid,
        config.fs_type.as_str()
    );

    if let Some(efi) = config.efi_partition() {
        let efi_uuid = get_uuid(&efi)?;
        content.push_str(&format!(
            "UUID={}    /boot/efi vfat    defaults,umask=0077    0 2\n",
            efi_uuid
        ));
    }

    fs::write(&fstab_path, &content)
        .map_err(|e| format!("Failed to write {}: {}", fstab_path, e))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Stage 6: Configure system
// ---------------------------------------------------------------------------
fn configure_system(config: &InstallConfig) -> Result<(), String> {
    log::log("Configuring system...");

    // Hostname
    let hostname_path = format!("{}/etc/hostname", TARGET_MNT);
    fs::write(&hostname_path, format!("{}\n", config.hostname))
        .map_err(|e| format!("Failed to write hostname: {}", e))?;

    // Hosts
    let hosts_path = format!("{}/etc/hosts", TARGET_MNT);
    let hosts_content = format!(
        "127.0.0.1   localhost\n::1         localhost\n127.0.1.1   {}\n",
        config.hostname
    );
    fs::write(&hosts_path, &hosts_content)
        .map_err(|e| format!("Failed to write hosts: {}", e))?;

    // Timezone
    if config.timezone != "UTC" {
        let tz_file = format!("/usr/share/zoneinfo/{}", config.timezone);
        if Path::new(&tz_file).is_file() {
            let localtime = format!("{}/etc/localtime", TARGET_MNT);
            let _ = fs::remove_file(&localtime);
            std::os::unix::fs::symlink(
                &format!("/usr/share/zoneinfo/{}", config.timezone),
                &localtime,
            )
            .map_err(|e| format!("Failed to symlink timezone: {}", e))?;
        }
    }

    // Fix /etc/passwd: restore the 'x' marker so login checks /etc/shadow
    let passwd_path = format!("{}/etc/passwd", TARGET_MNT);
    if let Ok(content) = fs::read_to_string(&passwd_path) {
        let fixed: String = content
            .lines()
            .map(|line| {
                if line.starts_with("root:") {
                    let parts: Vec<&str> = line.splitn(3, ':').collect();
                    if parts.len() >= 3 {
                        format!("root:x:{}", parts[2])
                    } else {
                        line.to_string()
                    }
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let fixed = if content.ends_with('\n') && !fixed.ends_with('\n') {
            fixed + "\n"
        } else {
            fixed
        };
        fs::write(&passwd_path, &fixed)
            .map_err(|e| format!("Failed to write passwd: {}", e))?;
    }

    // Replace agetty with greetd service
    let agetty_path = format!("{}/etc/dynamod/services/agetty-tty1.toml", TARGET_MNT);
    let _ = fs::remove_file(&agetty_path);

    let greetd_dir = format!("{}/etc/dynamod/services", TARGET_MNT);
    fs::create_dir_all(&greetd_dir)
        .map_err(|e| format!("mkdir {}: {}", greetd_dir, e))?;
    let greetd_path = format!("{}/greetd.toml", greetd_dir);
    fs::write(&greetd_path, GREETD_SERVICE)
        .map_err(|e| format!("Failed to write greetd service: {}", e))?;

    // Remove live-environment artifacts
    let live_profile = format!("{}/etc/profile.d/eclipse-live.sh", TARGET_MNT);
    let _ = fs::remove_file(&live_profile);

    // resolv.conf
    let resolv_path = format!("{}/etc/resolv.conf", TARGET_MNT);
    fs::write(&resolv_path, "nameserver 8.8.8.8\n")
        .map_err(|e| format!("Failed to write resolv.conf: {}", e))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Stage 7: Accounts (bind-mount proc/dev/sys, then chroot operations)
// ---------------------------------------------------------------------------
fn setup_accounts(config: &InstallConfig) -> Result<(), String> {
    // Bind-mount /proc, /dev, /sys
    for dir in &["proc", "dev", "sys"] {
        let src = format!("/{}", dir);
        let dst = format!("{}/{}", TARGET_MNT, dir);
        let _ = run_cmd("mount", &["--bind", &src, &dst]);
    }

    // Set root password
    log::log("Setting root password...");
    if let Some(ref pass) = config.root_password {
        run_chroot_stdin("chpasswd", &[], &format!("root:{}\n", pass))?;
    } else {
        run_chroot("passwd", &["-d", "root"])?;
    }

    // Create user account
    log::log(&format!("Creating user {}...", config.username));
    run_chroot(
        "useradd",
        &[
            "-m",
            "-G",
            USER_GROUPS,
            "-s",
            USER_SHELL,
            &config.username,
        ],
    )?;

    // Set user password
    if let Some(ref pass) = config.user_password {
        run_chroot_stdin(
            "chpasswd",
            &[],
            &format!("{}:{}\n", config.username, pass),
        )?;
    } else {
        run_chroot("passwd", &["-d", &config.username])?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Stage 8: Install GRUB
// ---------------------------------------------------------------------------
fn install_grub(config: &InstallConfig) -> Result<(), String> {
    log::log("Installing GRUB...");

    if config.uefi {
        run_chroot(
            "grub-install",
            &[
                "--target=x86_64-efi",
                "--efi-directory=/boot/efi",
                "--bootloader-id=eclipse",
                "--removable",
            ],
        )
        .map_err(|e| {
            log::log(&format!("grub-install (UEFI) failed: {}", e));
            e
        })?;
    } else {
        let mods_arg = format!("--modules={}", GRUB_BIOS_MODULES);
        let disk = config.disk.clone();

        // Try chroot grub-install first
        let result = run_chroot(
            "grub-install",
            &["--target=i386-pc", &mods_arg, &disk],
        );

        if result.is_err() {
            log::log("chroot grub-install failed, trying from live env...");
            let boot_dir = format!("--boot-directory={}/boot", TARGET_MNT);
            let _ = run_cmd(
                "grub-install",
                &["--target=i386-pc", &boot_dir, &mods_arg, &disk],
            );
        }

        // Fallback: copy i386-pc modules if normal.mod is missing
        let normal_mod = format!("{}/boot/grub/i386-pc/normal.mod", TARGET_MNT);
        if !Path::new(&normal_mod).exists() {
            let search_dirs = [
                format!("{}/usr/lib/grub/i386-pc", TARGET_MNT),
                "/usr/lib/grub/i386-pc".to_string(),
                format!("{}/usr/share/grub/i386-pc", TARGET_MNT),
                "/usr/share/grub/i386-pc".to_string(),
            ];

            for dir in &search_dirs {
                let check = format!("{}/normal.mod", dir);
                if Path::new(&check).exists() {
                    log::log(&format!("Copying GRUB modules from {}", dir));
                    let dest = format!("{}/boot/grub/i386-pc", TARGET_MNT);
                    fs::create_dir_all(&dest)
                        .map_err(|e| format!("mkdir {}: {}", dest, e))?;

                    if let Ok(entries) = fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if name.ends_with(".mod") || name.ends_with(".lst") {
                                let src = entry.path();
                                let dst = format!("{}/{}", dest, name);
                                let _ = fs::copy(&src, &dst);
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Stage 9: Write grub.cfg
// ---------------------------------------------------------------------------
fn detect_kernel_version() -> Result<String, String> {
    let boot_dir = format!("{}/boot", TARGET_MNT);
    if let Ok(entries) = fs::read_dir(&boot_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("vmlinuz-") && entry.path().is_file() {
                return Ok(name.strip_prefix("vmlinuz-").unwrap().to_string());
            }
        }
    }
    Err("Could not detect kernel version (no vmlinuz-* in /boot)".to_string())
}

fn write_grub_cfg(config: &InstallConfig, kver: &str, root_uuid: &str) -> Result<(), String> {
    log::log("Writing grub.cfg...");

    // Use grub2 directory if it exists, otherwise grub
    let grub_dir = if Path::new(&format!("{}/boot/grub2", TARGET_MNT)).is_dir() {
        format!("{}/boot/grub2", TARGET_MNT)
    } else {
        format!("{}/boot/grub", TARGET_MNT)
    };
    fs::create_dir_all(&grub_dir).map_err(|e| format!("mkdir {}: {}", grub_dir, e))?;

    let fs_type = config.fs_type.as_str();
    let cfg = format!(
        r#"set timeout=5
set default=0

menuentry "Eclipse Linux" {{
    linux /boot/vmlinuz-{kver} root=UUID={root_uuid} rootfstype={fs_type} rootwait rdinit=/sbin/dynamod-init init=/sbin/dynamod-init quiet
    initrd /boot/initramfs-{kver}.gz
}}

menuentry "Eclipse Linux (serial console)" {{
    linux /boot/vmlinuz-{kver} root=UUID={root_uuid} rootfstype={fs_type} rootwait rdinit=/sbin/dynamod-init init=/sbin/dynamod-init console=ttyS0,115200
    initrd /boot/initramfs-{kver}.gz
}}

menuentry "Eclipse Linux (recovery)" {{
    linux /boot/vmlinuz-{kver} root=UUID={root_uuid} rootfstype={fs_type} rootwait rdinit=/sbin/dynamod-init init=/sbin/dynamod-init single
    initrd /boot/initramfs-{kver}.gz
}}
"#,
        kver = kver,
        root_uuid = root_uuid,
        fs_type = fs_type,
    );

    let cfg_path = format!("{}/grub.cfg", grub_dir);
    fs::write(&cfg_path, &cfg).map_err(|e| format!("Failed to write grub.cfg: {}", e))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Stage 10: Build initramfs
//
// Replicates the shell script's initramfs assembly: copies dynamod-init,
// busybox (+ symlinks), kmod with its required shared libraries, and kernel
// modules, then packs everything with cpio newc + gzip -9.
// ---------------------------------------------------------------------------
fn build_initramfs(kver: &str) -> Result<(), String> {
    log::log("Building installed initramfs...");

    let output = run_cmd("mktemp", &["-d"])?;
    let staging = String::from_utf8_lossy(&output.stdout).trim().to_string();
    log::log(&format!("Initramfs staging dir: {}", staging));

    let result = build_initramfs_inner(kver, &staging);

    log::log("Cleaning up initramfs staging dir...");
    let _ = run_cmd("rm", &["-rf", &staging]);
    log::log("Initramfs staging dir cleaned up");

    result
}

fn build_initramfs_inner(kver: &str, staging: &str) -> Result<(), String> {
    log::log("initramfs: creating directory structure...");
    for dir in &[
        "sbin", "bin", "dev", "proc", "sys", "newroot", "etc", "lib",
    ] {
        fs::create_dir_all(format!("{}/{}", staging, dir))
            .map_err(|e| format!("mkdir in staging: {}", e))?;
    }

    log::log("initramfs: copying dynamod-init...");
    let dynamod_init_src = format!("{}/sbin/dynamod-init", TARGET_MNT);
    let dynamod_init_dst = format!("{}/sbin/dynamod-init", staging);
    fs::copy(&dynamod_init_src, &dynamod_init_dst)
        .map_err(|e| format!("copy dynamod-init: {}", e))?;

    log::log("initramfs: copying busybox...");
    let busybox_src = find_first_existing(&[
        &format!("{}/usr/bin/busybox", TARGET_MNT),
        &format!("{}/bin/busybox", TARGET_MNT),
    ]);
    if let Some(bb) = busybox_src {
        let bb_dst = format!("{}/bin/busybox", staging);
        fs::copy(&bb, &bb_dst).map_err(|e| format!("copy busybox: {}", e))?;
        for cmd in BUSYBOX_SYMLINKS {
            let link = format!("{}/bin/{}", staging, cmd);
            let _ = std::os::unix::fs::symlink("busybox", &link);
        }
    } else {
        log::log("initramfs: busybox not found, skipping");
    }

    log::log("initramfs: copying kmod and libraries...");
    let kmod_src = find_first_existing(&[
        &format!("{}/usr/bin/kmod", TARGET_MNT),
        &format!("{}/bin/kmod", TARGET_MNT),
    ]);

    if let Some(km) = kmod_src {
        let kmod_dst = format!("{}/bin/kmod", staging);
        fs::copy(&km, &kmod_dst).map_err(|e| format!("copy kmod: {}", e))?;
        let _ = std::os::unix::fs::symlink("kmod", &format!("{}/bin/modprobe", staging));
        let _ = std::os::unix::fs::symlink(
            "../bin/modprobe",
            &format!("{}/sbin/modprobe", staging),
        );

        fs::create_dir_all(format!("{}/lib", staging)).ok();
        fs::create_dir_all(format!("{}/usr/lib64", staging)).ok();
        fs::create_dir_all(format!("{}/usr/lib", staging)).ok();

        copy_glob_files(
            &format!("{}/lib", TARGET_MNT),
            "ld-musl-",
            &format!("{}/lib", staging),
        );

        for libc_candidate in &[
            format!("{}/usr/lib64/libc.so", TARGET_MNT),
            format!("{}/usr/lib/libc.so", TARGET_MNT),
            format!("{}/lib/libc.so", TARGET_MNT),
        ] {
            if Path::new(libc_candidate).is_file() {
                let relative = libc_candidate
                    .strip_prefix(TARGET_MNT)
                    .unwrap_or(libc_candidate);
                let dest = format!("{}{}", staging, relative);
                if let Some(parent) = Path::new(&dest).parent() {
                    fs::create_dir_all(parent).ok();
                }
                let _ = fs::copy(libc_candidate, &dest);
                break;
            }
        }

        for pattern in INITRAMFS_LIB_PATTERNS {
            for search_dir in &[
                format!("{}/usr/lib", TARGET_MNT),
                format!("{}/lib", TARGET_MNT),
                format!("{}/usr/lib64", TARGET_MNT),
            ] {
                copy_glob_files_with_prefix(search_dir, pattern, staging, TARGET_MNT);
            }
        }
    } else {
        log::log("initramfs: kmod not found, using busybox modprobe symlinks");
        let _ = std::os::unix::fs::symlink("busybox", &format!("{}/bin/modprobe", staging));
        let _ = std::os::unix::fs::symlink(
            "../bin/modprobe",
            &format!("{}/sbin/modprobe", staging),
        );
    }

    if !kver.is_empty() {
        let mod_src = format!("{}/lib/modules/{}", TARGET_MNT, kver);
        if Path::new(&mod_src).is_dir() {
            log::log(&format!("initramfs: copying kernel modules from {}...", mod_src));
            let mod_dst = format!("{}/lib/modules/{}", staging, kver);
            fs::create_dir_all(&format!("{}/lib/modules", staging))
                .map_err(|e| format!("mkdir modules: {}", e))?;
            run_cmd("cp", &["-a", &mod_src, &mod_dst])?;
            log::log("initramfs: kernel modules copied");
        } else {
            log::log(&format!("initramfs: no kernel modules at {}", mod_src));
        }
    }

    // Build the cpio.gz archive in two separate steps to avoid any
    // pipeline fd-inheritance issues that can cause deadlocks when
    // spawning multi-process pipes from a threaded Rust program.
    let initramfs_path = format!("{}/boot/initramfs-{}.gz", TARGET_MNT, kver);
    let cpio_tmp = format!("{}/initramfs.cpio", staging);

    log::log("initramfs: creating cpio archive...");
    let cpio_cmd = format!(
        "cd '{}' && find . -print0 | cpio --null -o --format=newc > '{}' 2>/dev/null",
        staging, cpio_tmp
    );
    run_cmd("sh", &["-c", &cpio_cmd])?;
    log::log("initramfs: cpio archive created");

    log::log("initramfs: compressing with gzip...");
    run_cmd("gzip", &["-9", "-f", &cpio_tmp])?;
    log::log("initramfs: gzip complete");

    let gzipped = format!("{}.gz", cpio_tmp);
    fs::rename(&gzipped, &initramfs_path)
        .map_err(|e| format!("Failed to move initramfs to {}: {}", initramfs_path, e))?;
    log::log(&format!("initramfs: moved to {}", initramfs_path));

    Ok(())
}

/// Copy files from `src_dir` whose names start with `prefix` into `dst_dir`,
/// preserving symlinks.
fn copy_glob_files(src_dir: &str, prefix: &str, dst_dir: &str) {
    if let Ok(entries) = fs::read_dir(src_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(prefix) {
                let src = entry.path();
                let dst = format!("{}/{}", dst_dir, name);
                let _ = run_cmd("cp", &["-a", src.to_str().unwrap_or(""), &dst]);
            }
        }
    }
}

/// Copy .so files matching a prefix, preserving their relative path under the target mount.
fn copy_glob_files_with_prefix(search_dir: &str, pattern: &str, staging: &str, target_mnt: &str) {
    if let Ok(entries) = fs::read_dir(search_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(pattern) && name.contains(".so") {
                let src = entry.path();
                let relative = src
                    .to_str()
                    .unwrap_or("")
                    .strip_prefix(target_mnt)
                    .unwrap_or("");
                let dest = format!("{}{}", staging, relative);
                if let Some(parent) = Path::new(&dest).parent() {
                    fs::create_dir_all(parent).ok();
                }
                let _ = run_cmd(
                    "cp",
                    &["-a", src.to_str().unwrap_or(""), &dest],
                );
            }
        }
    }
}

fn find_first_existing(candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .find(|p| Path::new(p).is_file())
        .map(|p| p.to_string())
}

// ---------------------------------------------------------------------------
// Stage 11: Cleanup
// ---------------------------------------------------------------------------
pub fn cleanup_mounts(config: &InstallConfig) {
    log::log("Cleanup...");
    for dir in &["proc", "dev", "sys"] {
        let target = format!("{}/{}", TARGET_MNT, dir);
        let _ = Command::new("umount").arg(&target).output();
    }
    if config.uefi {
        let efi = format!("{}/boot/efi", TARGET_MNT);
        let _ = Command::new("umount").arg(&efi).output();
    }
    let _ = Command::new("umount").arg(TARGET_MNT).output();
}

/// Emergency cleanup (called from signal handler context) - unmounts everything.
pub fn emergency_cleanup() {
    for dir in &["proc", "dev", "sys", "boot/efi"] {
        let target = format!("{}/{}", TARGET_MNT, dir);
        let _ = Command::new("umount").arg(&target).output();
    }
    let _ = Command::new("umount").arg(TARGET_MNT).output();
}
