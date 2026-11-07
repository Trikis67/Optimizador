//! Historial de cambios: cada acción que modifica la PC queda registrada para que el usuario
//! sepa exactamente qué hizo la app y cuándo. Se guarda en %LOCALAPPDATA%\Optimizador\historial.json.

use crate::sysutil;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

const MAX: usize = 500;
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Clone)]
pub struct Entry {
    /// Segundos desde 1970 (UTC)
    pub ts: u64,
    pub area: String,
    pub action: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default = "yes")]
    pub ok: bool,
}

fn yes() -> bool {
    true
}

fn file() -> PathBuf {
    sysutil::env_path("LOCALAPPDATA").unwrap_or_default().join(r"Optimizador\historial.json")
}

fn load() -> Vec<Entry> {
    std::fs::read_to_string(file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Agrega una entrada. Nunca falla: si no se puede escribir, la acción igual sigue.
pub fn log(area: &str, action: &str, detail: impl Into<String>, ok: bool) {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = load();
    list.push(Entry { ts: now(), area: area.into(), action: action.into(), detail: detail.into(), ok });
    if list.len() > MAX {
        let extra = list.len() - MAX;
        list.drain(..extra);
    }
    let path = file();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(&list) {
        let _ = std::fs::write(path, json);
    }
}

/// Registra el resultado de una acción y lo devuelve sin cambios.
pub fn track<T>(area: &str, action: &str, detail: impl Into<String>, r: Result<T, String>) -> Result<T, String> {
    match &r {
        Ok(_) => log(area, action, detail, true),
        Err(e) => log(area, action, format!("{} — falló: {e}", detail.into()), false),
    }
    r
}

pub fn list() -> Vec<Entry> {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut l = load();
    l.reverse();
    l
}

pub fn clear() -> Result<(), String> {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    match std::fs::remove_file(file()) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn folder() -> PathBuf {
    file().parent().map(|p| p.to_path_buf()).unwrap_or_default()
}
