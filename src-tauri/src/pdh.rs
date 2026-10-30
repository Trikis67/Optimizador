//! Contadores de rendimiento de Windows (PDH). Son los mismos datos que usa el Administrador de tareas
//! para la GPU, el disco y la red.

use crate::sysutil::{from_wide_ptr, wide};
use std::collections::HashMap;
use std::ptr::null;

const PDH_FMT_DOUBLE: u32 = 0x0000_0200;
const PDH_FMT_NOCAP100: u32 = 0x0000_8000;
const PDH_MORE_DATA: u32 = 0x8000_07D2;
const PDH_CSTATUS_VALID_DATA: u32 = 0;
const PDH_CSTATUS_NEW_DATA: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct FmtValue {
    status: u32,
    // La unión de PDH_FMT_COUNTERVALUE está alineada a 8 bytes; con PDH_FMT_DOUBLE es un f64.
    value: f64,
}

#[repr(C)]
struct FmtItem {
    name: *const u16,
    value: FmtValue,
}

#[link(name = "pdh")]
extern "system" {
    fn PdhOpenQueryW(source: *const u16, user: usize, query: *mut isize) -> u32;
    fn PdhAddEnglishCounterW(query: isize, path: *const u16, user: usize, counter: *mut isize) -> u32;
    fn PdhCollectQueryData(query: isize) -> u32;
    fn PdhGetFormattedCounterValue(counter: isize, format: u32, typ: *mut u32, value: *mut FmtValue) -> u32;
    fn PdhGetFormattedCounterArrayW(
        counter: isize,
        format: u32,
        buffer_size: *mut u32,
        item_count: *mut u32,
        items: *mut FmtItem,
    ) -> u32;
    fn PdhCloseQuery(query: isize) -> u32;
}

pub struct Pdh {
    query: isize,
    counters: HashMap<&'static str, isize>,
}

// Los identificadores de PDH se pueden usar desde cualquier hilo; siempre accedemos con un Mutex.
unsafe impl Send for Pdh {}

impl Pdh {
    pub fn new() -> Option<Pdh> {
        let mut query = 0isize;
        let rc = unsafe { PdhOpenQueryW(null(), 0, &mut query) };
        if rc != 0 {
            return None;
        }
        Some(Pdh { query, counters: HashMap::new() })
    }

    /// Agrega un contador usando su nombre en inglés (funciona en Windows en español).
    pub fn add(&mut self, key: &'static str, path: &str) -> bool {
        let p = wide(path);
        let mut counter = 0isize;
        let rc = unsafe { PdhAddEnglishCounterW(self.query, p.as_ptr(), 0, &mut counter) };
        if rc == 0 {
            self.counters.insert(key, counter);
            true
        } else {
            false
        }
    }

    pub fn collect(&self) {
        unsafe {
            PdhCollectQueryData(self.query);
        }
    }

    /// Valor de un contador sin comodines.
    pub fn value(&self, key: &str) -> Option<f64> {
        let counter = *self.counters.get(key)?;
        let mut v = FmtValue { status: 0, value: 0.0 };
        let rc = unsafe { PdhGetFormattedCounterValue(counter, PDH_FMT_DOUBLE | PDH_FMT_NOCAP100, std::ptr::null_mut(), &mut v) };
        if rc == 0 && (v.status == PDH_CSTATUS_VALID_DATA || v.status == PDH_CSTATUS_NEW_DATA) {
            Some(v.value)
        } else {
            None
        }
    }

    /// Valores de un contador con comodín `(*)`: lista de (instancia, valor).
    pub fn array(&self, key: &str) -> Vec<(String, f64)> {
        let Some(&counter) = self.counters.get(key) else { return Vec::new() };
        let fmt = PDH_FMT_DOUBLE | PDH_FMT_NOCAP100;
        let mut size = 0u32;
        let mut count = 0u32;
        let rc = unsafe { PdhGetFormattedCounterArrayW(counter, fmt, &mut size, &mut count, std::ptr::null_mut()) };
        if rc != PDH_MORE_DATA || size == 0 {
            return Vec::new();
        }
        // Un solo bloque de memoria: primero los elementos y después los nombres a los que apuntan.
        let mut buf = vec![0u64; (size as usize + 7) / 8 + 1];
        let items = buf.as_mut_ptr() as *mut FmtItem;
        let rc = unsafe { PdhGetFormattedCounterArrayW(counter, fmt, &mut size, &mut count, items) };
        if rc != 0 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count as usize {
            let item = unsafe { &*items.add(i) };
            if item.value.status == PDH_CSTATUS_VALID_DATA || item.value.status == PDH_CSTATUS_NEW_DATA {
                let name = unsafe { from_wide_ptr(item.name) };
                out.push((name, item.value.value));
            }
        }
        out
    }
}

impl Drop for Pdh {
    fn drop(&mut self) {
        unsafe {
            PdhCloseQuery(self.query);
        }
    }
}

/// Uso de GPU como lo calcula el Administrador de tareas: por cada tipo de motor (3D, Copy, Video…)
/// se suma el uso de todos los procesos y se toma el motor más ocupado.
/// Devuelve (uso total %, uso por PID).
pub fn gpu_usage(engines: &[(String, f64)]) -> (f64, HashMap<u32, f64>) {
    let mut by_engine: HashMap<String, f64> = HashMap::new();
    let mut by_pid_engine: HashMap<(u32, String), f64> = HashMap::new();
    for (name, v) in engines {
        // Formato: pid_1234_luid_0x..._0x..._phys_0_eng_0_engtype_3D
        let engtype = name.rsplit_once("engtype_").map(|(_, t)| t.to_string()).unwrap_or_default();
        let luid = name.split("_luid_").nth(1).and_then(|s| s.split("_phys").next()).unwrap_or("").to_string();
        let key = format!("{luid}|{engtype}");
        *by_engine.entry(key.clone()).or_insert(0.0) += v;
        if let Some(pid) = name.strip_prefix("pid_").and_then(|s| s.split('_').next()).and_then(|s| s.parse::<u32>().ok()) {
            *by_pid_engine.entry((pid, key)).or_insert(0.0) += v;
        }
    }
    let total = by_engine.values().cloned().fold(0.0, f64::max).min(100.0);
    let mut by_pid: HashMap<u32, f64> = HashMap::new();
    for ((pid, _), v) in by_pid_engine {
        let e = by_pid.entry(pid).or_insert(0.0);
        if v > *e {
            *e = v.min(100.0);
        }
    }
    (total, by_pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calcula_gpu() {
        let data = vec![
            ("pid_100_luid_0x0_0x1_phys_0_eng_0_engtype_3D".to_string(), 30.0),
            ("pid_200_luid_0x0_0x1_phys_0_eng_0_engtype_3D".to_string(), 25.0),
            ("pid_200_luid_0x0_0x1_phys_0_eng_5_engtype_VideoDecode".to_string(), 40.0),
        ];
        let (total, pids) = gpu_usage(&data);
        assert_eq!(total, 55.0);
        assert_eq!(pids[&200], 40.0);
        assert_eq!(pids[&100], 30.0);
    }
}
