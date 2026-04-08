use std::fmt;

pub const BACKTITLE: &str = "Eclipse Linux";
pub const LOG_PATH: &str = "/tmp/eclipse-install.log";
pub const TARGET_MNT: &str = "/mnt/eclipse-target";
pub const DEFAULT_HOSTNAME: &str = "eclipse";
pub const EFI_LABEL: &str = "ECLIPSE_EFI";
pub const ROOT_LABEL: &str = "eclipse-root";
pub const GRUB_BIOS_MODULES: &str =
    "part_gpt part_msdos ext2 btrfs xfs fat normal configfile linux search echo test all_video";
pub const USER_GROUPS: &str = "wheel,audio,video,input,_seatd";
pub const USER_SHELL: &str = "/usr/bin/fish";
pub const SQUASHFS_CANDIDATES: &[&str] = &["/run/dynamod/live/squash", "/run/rootfsbase", "/"];
pub const RSYNC_EXCLUDES: &[&str] = &["/run/*", "/tmp/*", "/proc/*", "/sys/*", "/dev/*"];

pub const TIMEZONE_REGIONS: &[&str] = &[
    "Africa",
    "America",
    "Antarctica",
    "Arctic",
    "Asia",
    "Atlantic",
    "Australia",
    "Europe",
    "Indian",
    "Pacific",
    "US",
];

/// Profile script that auto-starts the niri Wayland session on tty1.
/// Replaces greetd which has session-worker hangs in seatd/dynamod environments.
pub const NIRI_AUTOSTART_PROFILE: &str = r#"# Auto-start Eclipse niri session on tty1 after login
if [ "$(tty)" = "/dev/tty1" ] && [ -z "$WAYLAND_DISPLAY" ]; then
    exec /usr/bin/eclipse-niri-session
fi
"#;

/// Busybox symlinks created inside the initramfs staging directory.
pub const BUSYBOX_SYMLINKS: &[&str] =
    &["sh", "mount", "umount", "losetup", "blkid", "switch_root"];

/// Shared library name prefixes copied into the initramfs for kmod.
pub const INITRAMFS_LIB_PATTERNS: &[&str] = &["libzstd", "liblzma", "libcrypto", "libz"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsType {
    Ext4,
    Btrfs,
    Xfs,
}

impl FsType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FsType::Ext4 => "ext4",
            FsType::Btrfs => "btrfs",
            FsType::Xfs => "xfs",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            FsType::Ext4 => "ext4 (recommended, stable)",
            FsType::Btrfs => "Btrfs (snapshots, compression)",
            FsType::Xfs => "XFS (high performance)",
        }
    }

    pub const ALL: &[FsType] = &[FsType::Ext4, FsType::Btrfs, FsType::Xfs];
}

impl fmt::Display for FsType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Information about a detected block device.
#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub path: String,
    pub size_gb: u64,
    pub model: String,
}

impl fmt::Display for DiskInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} - {}GB - {}", self.path, self.size_gb, self.model)
    }
}

/// All values collected by the wizard, passed to the installation thread.
#[derive(Debug, Clone)]
pub struct InstallConfig {
    pub disk: String,
    pub disk_info: String,
    pub uefi: bool,
    pub fs_type: FsType,
    pub hostname: String,
    pub root_password: Option<String>,
    pub username: String,
    pub user_password: Option<String>,
    pub timezone: String,
    pub squashfs_mount: String,
}

impl InstallConfig {
    pub fn new(squashfs_mount: String) -> Self {
        Self {
            disk: String::new(),
            disk_info: String::new(),
            uefi: std::path::Path::new("/sys/firmware/efi").is_dir(),
            fs_type: FsType::Ext4,
            hostname: DEFAULT_HOSTNAME.to_string(),
            root_password: None,
            username: String::new(),
            user_password: None,
            timezone: "UTC".to_string(),
            squashfs_mount,
        }
    }

    /// Returns the partition device path for the given partition number.
    /// nvme devices use `p1`, `p2`, etc. while sd/vd use `1`, `2`, etc.
    pub fn partition_path(&self, num: u8) -> String {
        if self.disk.contains("nvme") {
            format!("{}p{}", self.disk, num)
        } else {
            format!("{}{}", self.disk, num)
        }
    }

    pub fn efi_partition(&self) -> Option<String> {
        if self.uefi {
            Some(self.partition_path(1))
        } else {
            None
        }
    }

    pub fn root_partition(&self) -> String {
        self.partition_path(2)
    }

    pub fn boot_mode_str(&self) -> &'static str {
        if self.uefi {
            "UEFI"
        } else {
            "BIOS (Legacy)"
        }
    }
}
