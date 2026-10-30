//! Limpieza de RAM, al estilo de Mem Reduct.

use crate::sysutil::wide;
use serde::Serialize;
use std::ffi::c_void;
use std::ptr::{null, null_mut};

#[repr(C)]
struct MemoryStatusEx {
    length: u32,
    memory_load: u32,
    total_phys: u64,
    avail_phys: u64,
    total_page_file: u64,
    avail_page_file: u64,
    total_virtual: u64,
    avail_virtual: u64,
    avail_extended_virtual: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Luid {
    low: u32,
    high: i32,
}

#[repr(C)]
struct TokenPrivileges {
    count: u32,
    luid: Luid,
    attributes: u32,
}

const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const PROCESS_SET_QUOTA: u32 = 0x0100;
const TOKEN_ADJUST_PRIVILEGES: u32 = 0x0020;
const TOKEN_QUERY: u32 = 0x0008;
const SE_PRIVILEGE_ENABLED: u32 = 0x0002;
const ERROR_NOT_ALL_ASSIGNED: u32 = 1300;
const SYSTEM_MEMORY_LIST_INFORMATION: i32 = 80;

// Comandos de SYSTEM_MEMORY_LIST_COMMAND
const MEMORY_EMPTY_WORKING_SETS: i32 = 2;
const MEMORY_FLUSH_MODIFIED_LIST: i32 = 3;
const MEMORY_PURGE_STANDBY_LIST: i32 = 4;
const MEMORY_PURGE_LOW_PRIORITY_STANDBY_LIST: i32 = 5;

#[link(name = "kernel32")]
extern "system" {
    fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
    fn CloseHandle(handle: isize) -> i32;
    fn K32EmptyWorkingSet(process: isize) -> i32;
    fn K32EnumProcesses(pids: *mut u32, cb: u32, needed: *mut u32) -> i32;
    fn GetCurrentProcess() -> isize;
    fn GetLastError() -> u32;
}

#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(process: isize, access: u32, token: *mut isize) -> i32;
    fn LookupPrivilegeValueW(system: *const u16, name: *const u16, luid: *mut Luid) -> i32;
    fn AdjustTokenPrivileges(
        token: isize,
        disable_all: i32,
        new_state: *const TokenPrivileges,
        len: u32,
        prev: *mut c_void,
        ret_len: *mut u32,
    ) -> i32;
}

#[link(name = "ntdll")]
extern "system" {
    fn NtSetSystemInformation(class: i32, info: *mut c_void, len: u32) -> i32;
}

#[derive(Serialize, Clone, Copy)]
pub struct MemStatus {
    pub total: u64,
    pub available: u64,
    pub load: u32,
    pub commit_total: u64,
    pub commit_avail: u64,
}

pub fn status() -> MemStatus {
    let mut m = MemoryStatusEx {
        length: std::mem::size_of::<MemoryStatusEx>() as u32,
        memory_load: 0,
        total_phys: 0,
        avail_phys: 0,
        total_page_file: 0,
        avail_page_file: 0,
        total_virtual: 0,
        avail_virtual: 0,
        avail_extended_virtual: 0,
    };
    unsafe {
        GlobalMemoryStatusEx(&mut m);
    }
    MemStatus {
        total: m.total_phys,
        available: m.avail_phys,
        load: m.memory_load,
        commit_total: m.total_page_file,
        commit_avail: m.avail_page_file,
    }
}

/// Activa un privilegio del proceso actual. Devuelve false si el usuario no lo tiene (no es admin).
fn enable_privilege(name: &str) -> bool {
    unsafe {
        let mut token = 0isize;
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut luid = Luid { low: 0, high: 0 };
        let n = wide(name);
        let ok = if LookupPrivilegeValueW(null(), n.as_ptr(), &mut luid) != 0 {
            let tp = TokenPrivileges { count: 1, luid, attributes: SE_PRIVILEGE_ENABLED };
            let r = AdjustTokenPrivileges(token, 0, &tp, 0, null_mut(), null_mut());
            r != 0 && GetLastError() != ERROR_NOT_ALL_ASSIGNED
        } else {
            false
        };
        CloseHandle(token);
        ok
    }
}

fn memory_list_command(cmd: i32) -> Result<(), String> {
    if !enable_privilege("SeProfileSingleProcessPrivilege") {
        return Err("Necesita permisos de administrador".into());
    }
    let mut c = cmd;
    let status = unsafe {
        NtSetSystemInformation(SYSTEM_MEMORY_LIST_INFORMATION, &mut c as *mut i32 as *mut c_void, std::mem::size_of::<i32>() as u32)
    };
    if status >= 0 {
        Ok(())
    } else {
        Err(format!("Windows rechazó la operación (0x{:08X})", status as u32))
    }
}

fn all_pids() -> Vec<u32> {
    let mut pids = vec![0u32; 4096];
    let mut needed = 0u32;
    unsafe {
        if K32EnumProcesses(pids.as_mut_ptr(), (pids.len() * 4) as u32, &mut needed) == 0 {
            return Vec::new();
        }
    }
    pids.truncate(needed as usize / 4);
    pids
}

/// Vacía la memoria de trabajo de un proceso. Devuelve true si se pudo.
pub fn trim_process(pid: u32) -> bool {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_QUOTA, 0, pid);
        if h == 0 {
            return false;
        }
        let ok = K32EmptyWorkingSet(h) != 0;
        CloseHandle(h);
        ok
    }
}

#[derive(Serialize)]
pub struct CleanResult {
    pub freed: i64,
    pub before: u64,
    pub after: u64,
    pub steps: Vec<String>,
    pub warnings: Vec<String>,
}

/// Opciones: "working_sets", "standby", "standby_low", "modified".
pub fn clean(options: &[String]) -> CleanResult {
    let before = status().available;
    let mut steps = Vec::new();
    let mut warnings = Vec::new();
    let has = |o: &str| options.iter().any(|x| x == o);

    if has("working_sets") {
        // Con permisos de admin se puede hacer para todo el sistema de una vez.
        if memory_list_command(MEMORY_EMPTY_WORKING_SETS).is_ok() {
            steps.push("Memoria de trabajo de todos los procesos vaciada".into());
        } else {
            let pids = all_pids();
            let done = pids.iter().filter(|&&p| p > 4 && trim_process(p)).count();
            steps.push(format!("Memoria de trabajo vaciada en {done} de {} procesos", pids.len()));
            if done < pids.len() / 2 {
                warnings.push("Sin permisos de administrador solo se limpian tus propios programas".into());
            }
        }
    }
    if has("modified") {
        match memory_list_command(MEMORY_FLUSH_MODIFIED_LIST) {
            Ok(()) => steps.push("Lista modificada escrita a disco".into()),
            Err(e) => warnings.push(format!("Lista modificada: {e}")),
        }
    }
    if has("standby") {
        match memory_list_command(MEMORY_PURGE_STANDBY_LIST) {
            Ok(()) => steps.push("Lista standby (caché) liberada".into()),
            Err(e) => warnings.push(format!("Lista standby: {e}")),
        }
    } else if has("standby_low") {
        match memory_list_command(MEMORY_PURGE_LOW_PRIORITY_STANDBY_LIST) {
            Ok(()) => steps.push("Caché de baja prioridad liberada".into()),
            Err(e) => warnings.push(format!("Caché de baja prioridad: {e}")),
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(400));
    let after = status().available;
    CleanResult { freed: after as i64 - before as i64, before, after, steps, warnings }
}
