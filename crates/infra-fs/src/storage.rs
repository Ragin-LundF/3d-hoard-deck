//! Tells whether a folder sits on a network share or in a cloud-sync folder, where SQLite files get corrupted.

use std::path::Path;
use std::process::Command;

use hoarddeck_application::ports::{StorageInspector, StorageKind};

const NETWORK_TYPES: [&str; 9] = ["smbfs", "afpfs", "nfs", "nfs4", "cifs", "smb3", "webdav", "fuse.sshfs", "davfs"];
const CLOUD_MARKERS: [&str; 6] = [
    "/Library/CloudStorage/",
    "/Library/Mobile Documents/",
    "/Dropbox/",
    "/OneDrive",
    "/Google Drive/",
    "/iCloudDrive/",
];

/// Uses `df`, `mount` and `/proc/mounts`; UNC paths count as network on Windows.
pub struct SystemStorage;

impl StorageInspector for SystemStorage {
    fn kind(&self, dir: &Path) -> StorageKind {
        let text = format!("{}/", dir.display());
        if CLOUD_MARKERS.iter().any(|marker| text.contains(marker)) {
            return StorageKind::CloudSync;
        }
        if text.starts_with("\\\\") || is_mapped_network_drive(&text) {
            return StorageKind::Network;
        }
        match filesystem_type(dir) {
            Some(kind) if NETWORK_TYPES.contains(&kind.as_str()) => StorageKind::Network,
            _ => StorageKind::Local,
        }
    }
}

/// Windows: `Z:\…` mapped to a share shows up in `net use` with a `\\server\share` target.
fn is_mapped_network_drive(path: &str) -> bool {
    let bytes = path.as_bytes();
    if bytes.len() < 2 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' {
        return false;
    }
    let drive = path[..2].to_ascii_uppercase();
    Command::new("net").arg("use").output().is_ok_and(|output| {
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line.to_ascii_uppercase().contains(&drive) && line.contains("\\\\"))
    })
}

/// The type of the file system holding `dir` (or its nearest existing parent), e.g. `apfs` or `smbfs`.
fn filesystem_type(dir: &Path) -> Option<String> {
    let existing = dir.ancestors().find(|path| path.exists())?;
    let df = Command::new("df").arg("-P").arg(existing).output().ok()?;
    let mount_point = mount_point_from_df(&String::from_utf8_lossy(&df.stdout))?;
    let table = std::fs::read_to_string("/proc/mounts")
        .ok()
        .or_else(|| Command::new("mount").output().ok().map(|out| String::from_utf8_lossy(&out.stdout).into_owned()))?;
    filesystem_of(&table, &mount_point)
}

/// Last column of `df -P`, which may contain spaces; it follows the capacity column (`74%`).
fn mount_point_from_df(output: &str) -> Option<String> {
    let line = output.lines().nth(1)?;
    let mut fields = line.split_whitespace();
    fields.by_ref().find(|field| field.ends_with('%'))?;
    let mount_point = fields.collect::<Vec<_>>().join(" ");
    (!mount_point.is_empty()).then_some(mount_point)
}

/// Finds the type for `mount_point` in macOS `mount` output (`dev on /x (smbfs, …)`) or Linux `/proc/mounts`.
fn filesystem_of(table: &str, mount_point: &str) -> Option<String> {
    table.lines().find_map(|line| {
        if let Some((_, rest)) = line.split_once(" on ") {
            let (point, options) = rest.rsplit_once(" (")?;
            return (point == mount_point)
                .then(|| options.split([',', ')']).next().unwrap_or_default().trim().to_owned());
        }
        let fields: Vec<&str> = line.split(' ').collect();
        let point = fields.get(1)?.replace("\\040", " ");
        if point == mount_point { fields.get(2).map(|kind| (*kind).to_owned()) } else { None }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_mount_points_with_spaces_from_df() {
        let output = "Filesystem 1024-blocks Used Available Capacity Mounted on\n//j@nas/volume 100 50 50 50% /Volumes/3D Print\n";
        assert_eq!(mount_point_from_df(output).as_deref(), Some("/Volumes/3D Print"));
    }

    #[test]
    fn finds_network_types_in_macos_and_linux_mount_tables() {
        let macos = "/dev/disk3s5 on / (apfs, local)\n//j@nas/volume on /Volumes/3D Print (smbfs, nodev, nosuid)\n";
        assert_eq!(filesystem_of(macos, "/Volumes/3D Print").as_deref(), Some("smbfs"));
        assert_eq!(filesystem_of(macos, "/").as_deref(), Some("apfs"));
        let linux = "/dev/sda1 / ext4 rw 0 0\nnas:/share /mnt/3D\\040Print nfs4 rw 0 0\n";
        assert_eq!(filesystem_of(linux, "/mnt/3D Print").as_deref(), Some("nfs4"));
    }

    #[test]
    fn cloud_sync_folders_are_recognized_by_path() {
        let path = Path::new("/Users/j/Library/CloudStorage/Dropbox/hoard");
        assert_eq!(SystemStorage.kind(path), StorageKind::CloudSync);
    }
}
