//! Programas que se abren al iniciar Windows. Se activan/desactivan igual que en el Administrador de
//! tareas (claves StartupApproved), así que también se ven reflejados ahí.

use crate::sysutil::{self, env_path};
use crate::winreg::{self, Hive, RegValue};
use serde::Serialize;

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN32: &str = r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";

struct Location {
    id: &'static str,
    label: &'static str,
    hive: Hive,
    /// Clave de registro con los programas (None = carpeta de Inicio)
    run_key: Option<&'static str>,
    folder: Option<(&'static str, &'static str)>,
    approved: &'static str,
}

const LOCATIONS: &[Location] = &[
    Location { id: "hkcu_run", label: "Solo tu usuario", hive: Hive::Hkcu, run_key: Some(RUN), folder: None, approved: "Run" },
    Location { id: "hklm_run", label: "Todos los usuarios", hive: Hive::Hklm, run_key: Some(RUN), folder: None, approved: "Run" },
    Location { id: "hklm_run32", label: "Todos los usuarios (32 bits)", hive: Hive::Hklm, run_key: Some(RUN32), folder: None, approved: "Run32" },
    Location {
        id: "user_folder",
        label: "Carpeta Inicio (tu usuario)",
        hive: Hive::Hkcu,
        run_key: None,
        folder: Some(("APPDATA", r"Microsoft\Windows\Start Menu\Programs\Startup")),
        approved: "StartupFolder",
    },
    Location {
        id: "common_folder",
        label: "Carpeta Inicio (todos)",
        hive: Hive::Hklm,
        run_key: None,
        folder: Some(("ProgramData", r"Microsoft\Windows\Start Menu\Programs\StartUp")),
        approved: "StartupFolder",
    },
];

#[derive(Serialize)]
pub struct StartupEntry {
    pub id: String,
    pub name: String,
    pub command: String,
    pub location: String,
    pub enabled: bool,
    pub admin: bool,
}

fn is_enabled(hive: Hive, approved: &str, name: &str) -> bool {
    match winreg::get(hive, &format!(r"{APPROVED}\{approved}"), name) {
        // El primer byte impar (03, 07…) significa desactivado.
        Ok(Some(RegValue::Binary(b))) if !b.is_empty() => b[0] & 1 == 0,
        _ => true,
    }
}

pub fn list() -> Vec<StartupEntry> {
    let mut out = Vec::new();
    for loc in LOCATIONS {
        let mut push = |name: String, command: String| {
            let enabled = is_enabled(loc.hive, loc.approved, &name);
            out.push(StartupEntry {
                id: format!("{}|{}", loc.id, name),
                name,
                command,
                location: loc.label.into(),
                enabled,
                admin: loc.hive == Hive::Hklm,
            });
        };
        if let Some(key) = loc.run_key {
            for (name, value) in winreg::values(loc.hive, key) {
                let command = match value {
                    RegValue::Sz(s) => s,
                    _ => continue,
                };
                push(name, command);
            }
        }
        if let Some((var, rel)) = loc.folder {
            let Some(dir) = env_path(var).map(|p| p.join(rel)) else { continue };
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.eq_ignore_ascii_case("desktop.ini") {
                    continue;
                }
                push(name, e.path().to_string_lossy().into_owned());
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn set_enabled(id: &str, enabled: bool) -> Result<(), String> {
    let (loc_id, name) = id.split_once('|').ok_or("Elemento inválido")?;
    let loc = LOCATIONS.iter().find(|l| l.id == loc_id).ok_or("Ubicación desconocida")?;
    if loc.hive == Hive::Hklm && !sysutil::is_admin() {
        return Err("Este programa se inicia para todos los usuarios; necesitas permisos de administrador.".into());
    }
    let mut data = vec![0u8; 12];
    data[0] = if enabled { 0x02 } else { 0x03 };
    winreg::set(loc.hive, &format!(r"{APPROVED}\{}", loc.approved), name, &RegValue::Binary(data))
}
