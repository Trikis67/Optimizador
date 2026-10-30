//! Lista de unidades de disco.

use serde::Serialize;
use sysinfo::{DiskKind, Disks};

#[derive(Serialize)]
pub struct DiskInfo {
    pub mount: String,
    pub name: String,
    pub fs: String,
    pub kind: String,
    pub total: u64,
    pub free: u64,
    pub removable: bool,
}

pub fn disks() -> Vec<DiskInfo> {
    let disks = Disks::new_with_refreshed_list();
    let mut v: Vec<DiskInfo> = disks
        .list()
        .iter()
        .map(|d| DiskInfo {
            mount: d.mount_point().to_string_lossy().into_owned(),
            name: d.name().to_string_lossy().into_owned(),
            fs: d.file_system().to_string_lossy().into_owned(),
            kind: match d.kind() {
                DiskKind::SSD => "SSD".into(),
                DiskKind::HDD => "HDD".into(),
                _ => String::new(),
            },
            total: d.total_space(),
            free: d.available_space(),
            removable: d.is_removable(),
        })
        .filter(|d| d.total > 0)
        .collect();
    v.sort_by(|a, b| a.mount.cmp(&b.mount));
    v
}