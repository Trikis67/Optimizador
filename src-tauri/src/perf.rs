//! Monitor de rendimiento (tipo Administrador de tareas): CPU, GPU, RAM, disco, red y procesos.

use crate::pdh::{self, Pdh};
use serde::Serialize;
use std::collections::HashSet;
use sysinfo::{Pid, ProcessesToUpdate, System};

pub struct Perf {
    sys: System,
    pdh: Option<Pdh>,
}

#[derive(Serialize)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub mem: u64,
    pub gpu: f32,
}

#[derive(Serialize)]
pub struct Snapshot {
    pub cpu: f64,
    pub cores: Vec<f32>,
    pub cpu_mhz: f64,
    pub cpu_name: String,
    pub ram_total: u64,
    pub ram_used: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub gpu: f64,
    pub gpu_mem: u64,
    pub gpu_shared: u64,
    pub disk_active: f64,
    pub disk_read: f64,
    pub disk_write: f64,
    pub net_rx: f64,
    pub net_tx: f64,
    pub uptime: u64,
    pub proc_count: usize,
    pub processes: Vec<ProcInfo>,
}

#[derive(Serialize)]
pub struct MemDetail {
    pub standby: u64,
    pub modified: u64,
    pub committed: u64,
    pub commit_limit: u64,
}

impl Perf {
    pub fn new() -> Perf {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        let pdh = Pdh::new().map(|mut p| {
            p.add("cpu_util", r"\Processor Information(_Total)\% Processor Utility");
            p.add("cpu_perf", r"\Processor Information(_Total)\% Processor Performance");
            p.add("cpu_freq", r"\Processor Information(_Total)\Processor Frequency");
            p.add("gpu_engine", r"\GPU Engine(*)\Utilization Percentage");
            p.add("gpu_dedicated", r"\GPU Adapter Memory(*)\Dedicated Usage");
            p.add("gpu_shared", r"\GPU Adapter Memory(*)\Shared Usage");
            p.add("disk_idle", r"\PhysicalDisk(_Total)\% Idle Time");
            p.add("disk_read", r"\PhysicalDisk(_Total)\Disk Read Bytes/sec");
            p.add("disk_write", r"\PhysicalDisk(_Total)\Disk Write Bytes/sec");
            p.add("net_rx", r"\Network Interface(*)\Bytes Received/sec");
            p.add("net_tx", r"\Network Interface(*)\Bytes Sent/sec");
            p.add("standby_reserve", r"\Memory\Standby Cache Reserve Bytes");
            p.add("standby_normal", r"\Memory\Standby Cache Normal Priority Bytes");
            p.add("standby_core", r"\Memory\Standby Cache Core Bytes");
            p.add("modified", r"\Memory\Modified Page List Bytes");
            p.add("committed", r"\Memory\Committed Bytes");
            p.add("commit_limit", r"\Memory\Commit Limit");
            p.collect();
            p
        });
        Perf { sys, pdh }
    }

    fn val(&self, key: &str) -> f64 {
        self.pdh.as_ref().and_then(|p| p.value(key)).unwrap_or(0.0)
    }

    fn sum(&self, key: &str) -> f64 {
        self.pdh.as_ref().map(|p| p.array(key).iter().map(|(_, v)| v).sum()).unwrap_or(0.0)
    }

    fn max(&self, key: &str) -> f64 {
        self.pdh.as_ref().map(|p| p.array(key).iter().map(|(_, v)| *v).fold(0.0, f64::max)).unwrap_or(0.0)
    }

    pub fn snapshot(&mut self) -> Snapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        if let Some(p) = &self.pdh {
            p.collect();
        }

        let (gpu, gpu_by_pid) = match &self.pdh {
            Some(p) => pdh::gpu_usage(&p.array("gpu_engine")),
            None => (0.0, Default::default()),
        };

        let n_cpus = self.sys.cpus().len().max(1) as f32;
        let mut processes: Vec<ProcInfo> = self
            .sys
            .processes()
            .values()
            .filter(|p| p.pid().as_u32() != 0)
            .map(|p| {
                let pid = p.pid().as_u32();
                ProcInfo {
                    pid,
                    name: p.name().to_string_lossy().into_owned(),
                    cpu: p.cpu_usage() / n_cpus,
                    mem: p.memory(),
                    gpu: gpu_by_pid.get(&pid).copied().unwrap_or(0.0) as f32,
                }
            })
            .collect();
        processes.sort_unstable_by(|a, b| b.mem.cmp(&a.mem));

        let util = self.val("cpu_util");
        let cpu = if util > 0.0 { util.min(100.0) } else { self.sys.global_cpu_usage() as f64 };
        let base = self.val("cpu_freq");
        let perf = self.val("cpu_perf");
        let cpu_mhz = if base > 0.0 && perf > 0.0 { base * perf / 100.0 } else { base };

        Snapshot {
            cpu,
            cores: self.sys.cpus().iter().map(|c| c.cpu_usage()).collect(),
            cpu_mhz,
            cpu_name: self.sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
            ram_total: self.sys.total_memory(),
            ram_used: self.sys.used_memory(),
            swap_total: self.sys.total_swap(),
            swap_used: self.sys.used_swap(),
            gpu,
            gpu_mem: self.max("gpu_dedicated") as u64,
            gpu_shared: self.max("gpu_shared") as u64,
            disk_active: (100.0 - self.val("disk_idle")).clamp(0.0, 100.0),
            disk_read: self.val("disk_read"),
            disk_write: self.val("disk_write"),
            net_rx: self.sum("net_rx"),
            net_tx: self.sum("net_tx"),
            uptime: System::uptime(),
            proc_count: processes.len(),
            processes,
        }
    }

    /// Detalle de memoria (lista standby, modificada, memoria comprometida).
    pub fn mem_detail(&mut self) -> MemDetail {
        if let Some(p) = &self.pdh {
            p.collect();
        }
        MemDetail {
            standby: (self.val("standby_reserve") + self.val("standby_normal") + self.val("standby_core")) as u64,
            modified: self.val("modified") as u64,
            committed: self.val("committed") as u64,
            commit_limit: self.val("commit_limit") as u64,
        }
    }

    /// Procesos ordenados por memoria (para la página de RAM).
    pub fn top_memory(&mut self, n: usize) -> Vec<ProcInfo> {
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        let mut v: Vec<ProcInfo> = self
            .sys
            .processes()
            .values()
            .filter(|p| p.pid().as_u32() != 0)
            .map(|p| ProcInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                cpu: 0.0,
                mem: p.memory(),
                gpu: 0.0,
            })
            .collect();
        v.sort_unstable_by(|a, b| b.mem.cmp(&a.mem));
        v.truncate(n);
        v
    }

    pub fn kill(&mut self, pid: u32) -> Result<(), String> {
        if pid <= 4 || pid == std::process::id() {
            return Err("Ese proceso no se puede finalizar".into());
        }
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        let proc = self.sys.process(Pid::from_u32(pid)).ok_or("El proceso ya no existe")?;
        if proc.kill() {
            Ok(())
        } else {
            Err("Windows no permitió finalizar el proceso (puede necesitar permisos de administrador)".into())
        }
    }
}

/// Nombres (en minúsculas) de los procesos que están corriendo.
pub fn running_names() -> HashSet<String> {
    let mut s = System::new();
    s.refresh_processes(ProcessesToUpdate::All, true);
    s.processes().values().map(|p| p.name().to_string_lossy().to_lowercase()).collect()
}
