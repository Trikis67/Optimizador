//! Acceso mínimo al Registro de Windows (leer, escribir, borrar y listar valores).

use crate::sysutil::wide;
use serde::{Deserialize, Serialize};
use std::ffi::c_void;
use std::ptr::{null, null_mut};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hive {
    #[serde(rename = "HKLM")]
    Hklm,
    #[serde(rename = "HKCU")]
    Hkcu,
}

impl Hive {
    fn handle(self) -> isize {
        // Las constantes HKEY_* se extienden con signo en 64 bits.
        match self {
            Hive::Hklm => 0x8000_0002u32 as i32 as isize,
            Hive::Hkcu => 0x8000_0001u32 as i32 as isize,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum RegValue {
    Dword(u32),
    Sz(String),
    Binary(Vec<u8>),
}

const REG_SZ: u32 = 1;
const REG_EXPAND_SZ: u32 = 2;
const REG_BINARY: u32 = 3;
const REG_DWORD: u32 = 4;
const RRF_RT_ANY: u32 = 0x0000_ffff;
const KEY_READ: u32 = 0x2_0019;
const KEY_SET_VALUE: u32 = 0x0002;
const ERROR_FILE_NOT_FOUND: i32 = 2;
const ERROR_ACCESS_DENIED: i32 = 5;
const ERROR_MORE_DATA: i32 = 234;
const ERROR_NO_MORE_ITEMS: i32 = 259;

#[link(name = "advapi32")]
extern "system" {
    fn RegGetValueW(
        hkey: isize,
        sub_key: *const u16,
        value: *const u16,
        flags: u32,
        typ: *mut u32,
        data: *mut c_void,
        cb_data: *mut u32,
    ) -> i32;
    fn RegCreateKeyExW(
        hkey: isize,
        sub_key: *const u16,
        reserved: u32,
        class: *const u16,
        options: u32,
        sam: u32,
        security: *const c_void,
        result: *mut isize,
        disposition: *mut u32,
    ) -> i32;
    fn RegOpenKeyExW(hkey: isize, sub_key: *const u16, options: u32, sam: u32, result: *mut isize) -> i32;
    fn RegSetValueExW(hkey: isize, name: *const u16, reserved: u32, typ: u32, data: *const u8, cb: u32) -> i32;
    fn RegDeleteKeyValueW(hkey: isize, sub_key: *const u16, name: *const u16) -> i32;
    fn RegEnumValueW(
        hkey: isize,
        index: u32,
        name: *mut u16,
        cch_name: *mut u32,
        reserved: *mut u32,
        typ: *mut u32,
        data: *mut u8,
        cb_data: *mut u32,
    ) -> i32;
    fn RegCloseKey(hkey: isize) -> i32;
}

fn err(code: i32, what: &str) -> String {
    if code == ERROR_ACCESS_DENIED {
        "Esta acción necesita permisos de administrador. Usa «Reiniciar como administrador».".into()
    } else {
        format!("Error del Registro ({what}): código {code}")
    }
}

fn decode(typ: u32, buf: &[u8]) -> Option<RegValue> {
    match typ {
        REG_DWORD if buf.len() >= 4 => Some(RegValue::Dword(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]))),
        REG_SZ | REG_EXPAND_SZ => {
            let units: Vec<u16> = buf.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
            Some(RegValue::Sz(String::from_utf16_lossy(&units[..end])))
        }
        _ => Some(RegValue::Binary(buf.to_vec())),
    }
}

/// Lee un valor. `Ok(None)` si no existe.
pub fn get(hive: Hive, key: &str, name: &str) -> Result<Option<RegValue>, String> {
    let k = wide(key);
    let n = wide(name);
    let mut typ = 0u32;
    let mut size = 0u32;
    unsafe {
        let rc = RegGetValueW(hive.handle(), k.as_ptr(), n.as_ptr(), RRF_RT_ANY, &mut typ, null_mut(), &mut size);
        if rc == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if rc != 0 {
            return Err(err(rc, "leer"));
        }
        let mut buf = vec![0u8; size as usize + 2];
        let mut size2 = buf.len() as u32;
        let rc = RegGetValueW(
            hive.handle(),
            k.as_ptr(),
            n.as_ptr(),
            RRF_RT_ANY,
            &mut typ,
            buf.as_mut_ptr() as *mut c_void,
            &mut size2,
        );
        if rc == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if rc != 0 {
            return Err(err(rc, "leer"));
        }
        buf.truncate(size2 as usize);
        Ok(decode(typ, &buf))
    }
}

/// Escribe un valor (crea la clave si no existe).
pub fn set(hive: Hive, key: &str, name: &str, value: &RegValue) -> Result<(), String> {
    let (typ, data): (u32, Vec<u8>) = match value {
        RegValue::Dword(d) => (REG_DWORD, d.to_le_bytes().to_vec()),
        RegValue::Sz(s) => (REG_SZ, wide(s).iter().flat_map(|u| u.to_le_bytes()).collect()),
        RegValue::Binary(b) => (REG_BINARY, b.clone()),
    };
    let k = wide(key);
    let n = wide(name);
    unsafe {
        let mut hk: isize = 0;
        let rc = RegCreateKeyExW(hive.handle(), k.as_ptr(), 0, null(), 0, KEY_SET_VALUE, null(), &mut hk, null_mut());
        if rc != 0 {
            return Err(err(rc, "abrir"));
        }
        let rc = RegSetValueExW(hk, n.as_ptr(), 0, typ, data.as_ptr(), data.len() as u32);
        RegCloseKey(hk);
        if rc != 0 {
            return Err(err(rc, "escribir"));
        }
    }
    Ok(())
}

/// Borra un valor. No falla si ya no existía.
pub fn delete(hive: Hive, key: &str, name: &str) -> Result<(), String> {
    let k = wide(key);
    let n = wide(name);
    let rc = unsafe { RegDeleteKeyValueW(hive.handle(), k.as_ptr(), n.as_ptr()) };
    if rc == 0 || rc == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(err(rc, "borrar"))
    }
}

/// Escribe o borra según `value`.
pub fn put(hive: Hive, key: &str, name: &str, value: Option<&RegValue>) -> Result<(), String> {
    match value {
        Some(v) => set(hive, key, name, v),
        None => delete(hive, key, name),
    }
}

/// Lista todos los valores de una clave. Vacío si la clave no existe.
pub fn values(hive: Hive, key: &str) -> Vec<(String, RegValue)> {
    let mut out = Vec::new();
    let k = wide(key);
    unsafe {
        let mut hk: isize = 0;
        if RegOpenKeyExW(hive.handle(), k.as_ptr(), 0, KEY_READ, &mut hk) != 0 {
            return out;
        }
        let mut name = vec![0u16; 16_384];
        let mut data = vec![0u8; 8_192];
        let mut index = 0u32;
        loop {
            let mut cch = name.len() as u32;
            let mut cb = data.len() as u32;
            let mut typ = 0u32;
            let rc = RegEnumValueW(hk, index, name.as_mut_ptr(), &mut cch, null_mut(), &mut typ, data.as_mut_ptr(), &mut cb);
            if rc == ERROR_NO_MORE_ITEMS {
                break;
            }
            if rc == ERROR_MORE_DATA {
                data.resize(cb as usize + 2, 0);
                continue; // reintenta el mismo índice con un búfer más grande
            }
            if rc != 0 {
                break;
            }
            let n = String::from_utf16_lossy(&name[..cch as usize]);
            if let Some(v) = decode(typ, &data[..cb as usize]) {
                out.push((n, v));
            }
            index += 1;
        }
        RegCloseKey(hk);
    }
    out
}
