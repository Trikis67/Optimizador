// Evita que se abra una consola extra en Windows al usar la versión final.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod browsers;
mod cleaner;
mod drivers;
mod hw;
mod memory;
mod monitor;
mod pdh;
mod perf;
mod power;
mod scanner;
mod startup;
mod sysutil;
mod tools;
mod tweaks;
mod winreg;

use scanner::{Progress, ScanResult};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

struct AppState {
    scan: Mutex<Option<ScanResult>>,
    progress: Arc<Progress>,
    scanning: AtomicBool,
    perf: Mutex<perf::Perf>,
    hw: Mutex<Option<hw::Hw>>,
}

fn get_hw(state: &AppState) -> Result<hw::Hw, String> {
    let mut guard = state.hw.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(hw::detect()?);
    }
    Ok(guard.clone().unwrap_or_default())
}

// ---------- General ----------

#[derive(Serialize)]
struct AppInfo {
    admin: bool,
    version: String,
    dev: bool,
}

#[tauri::command]
fn app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        admin: sysutil::is_admin(),
        version: app.package_info().version.to_string(),
        dev: cfg!(debug_assertions),
    }
}

#[tauri::command]
fn relaunch_admin(app: AppHandle) -> Result<(), String> {
    sysutil::relaunch_as_admin()?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn open_target(target: String) -> Result<(), String> {
    sysutil::open_target(&target)
}

#[tauri::command]
async fn hardware(state: State<'_, AppState>) -> Result<hw::Hw, String> {
    get_hw(&state)
}

// ---------- Analizador de disco ----------

#[derive(Serialize, Clone)]
struct ScanProgress {
    files: u64,
    dirs: u64,
    bytes: u64,
    errors: u64,
}

#[derive(Serialize)]
struct ScanSummary {
    path: String,
    size: u64,
    files: u64,
    dirs: u64,
    errors: u64,
    seconds: f64,
    cancelled: bool,
}

fn snapshot(p: &Progress) -> ScanProgress {
    ScanProgress {
        files: p.files.load(Relaxed),
        dirs: p.dirs.load(Relaxed),
        bytes: p.bytes.load(Relaxed),
        errors: p.errors.load(Relaxed),
    }
}

#[tauri::command]
fn list_drives() -> Vec<monitor::DiskInfo> {
    monitor::disks()
}

#[tauri::command]
async fn start_scan(app: AppHandle, path: String) -> Result<ScanSummary, String> {
    let root_path = PathBuf::from(path.trim());
    if !root_path.is_dir() {
        return Err(format!("La carpeta no existe: {}", root_path.display()));
    }

    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        if state.scanning.swap(true, Relaxed) {
            return Err("Ya hay un análisis en curso".to_string());
        }
        *state.scan.lock().unwrap() = None;
        let progress = state.progress.clone();
        progress.reset();

        let done = Arc::new(AtomicBool::new(false));
        let reporter = {
            let (app, progress, done) = (app.clone(), progress.clone(), done.clone());
            std::thread::spawn(move || {
                while !done.load(Relaxed) {
                    let _ = app.emit("scan-progress", snapshot(&progress));
                    std::thread::sleep(Duration::from_millis(150));
                }
            })
        };

        let start = Instant::now();
        let root = scanner::scan_dir(&root_path, root_path.to_string_lossy().into(), &progress);
        done.store(true, Relaxed);
        let _ = reporter.join();

        let summary = ScanSummary {
            path: root_path.to_string_lossy().into_owned(),
            size: root.size,
            files: root.files,
            dirs: progress.dirs.load(Relaxed),
            errors: progress.errors.load(Relaxed),
            seconds: start.elapsed().as_secs_f64(),
            cancelled: progress.cancel.load(Relaxed),
        };
        *state.scan.lock().unwrap() = Some(ScanResult { root_path, root });
        state.scanning.store(false, Relaxed);
        Ok(summary)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn cancel_scan(state: State<AppState>) {
    state.progress.cancel.store(true, Relaxed);
}

#[tauri::command]
fn get_children(state: State<AppState>, segments: Vec<String>) -> Result<scanner::Listing, String> {
    let guard = state.scan.lock().unwrap();
    let res = guard.as_ref().ok_or("Primero analiza una unidad o carpeta")?;
    let node = scanner::find(&res.root, &segments).ok_or("Carpeta no encontrada")?;
    Ok(scanner::listing(&res.root_path, node, &segments, 1000))
}

#[tauri::command]
fn top_files(state: State<AppState>, limit: usize) -> Result<Vec<scanner::BigFile>, String> {
    let guard = state.scan.lock().unwrap();
    let res = guard.as_ref().ok_or("Primero analiza una unidad o carpeta")?;
    Ok(scanner::top_files(res, limit.min(1000)))
}

#[tauri::command]
fn ext_stats(state: State<AppState>) -> Result<Vec<scanner::ExtStat>, String> {
    let guard = state.scan.lock().unwrap();
    let res = guard.as_ref().ok_or("Primero analiza una unidad o carpeta")?;
    Ok(scanner::ext_stats(&res.root, 40))
}

fn full_path(res: &ScanResult, segments: &[String]) -> PathBuf {
    let mut p = res.root_path.clone();
    for s in segments {
        p.push(s);
    }
    p
}

#[tauri::command]
fn open_in_explorer(state: State<AppState>, segments: Vec<String>) -> Result<(), String> {
    let path = {
        let guard = state.scan.lock().unwrap();
        let res = guard.as_ref().ok_or("No hay análisis")?;
        full_path(res, &segments)
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("Solo disponible en Windows".into())
    }
}

#[tauri::command]
fn move_to_trash(state: State<AppState>, segments: Vec<String>) -> Result<u64, String> {
    if segments.is_empty() {
        return Err("No se puede borrar la carpeta raíz del análisis".into());
    }
    let mut guard = state.scan.lock().unwrap();
    let res = guard.as_mut().ok_or("No hay análisis")?;
    let path = full_path(res, &segments);
    trash::delete(&path).map_err(|e| format!("No se pudo enviar a la papelera: {e}"))?;
    let (bytes, _) = scanner::remove(&mut res.root, &segments).unwrap_or((0, 0));
    Ok(bytes)
}

// ---------- Energía ----------

#[tauri::command]
async fn get_power_plans() -> Result<Vec<power::PowerPlan>, String> {
    power::list()
}

#[tauri::command]
async fn set_power_plan(guid: String) -> Result<(), String> {
    power::set_active(&guid)
}

#[tauri::command]
async fn enable_ultimate_plan() -> Result<(), String> {
    power::enable_ultimate()
}

#[tauri::command]
async fn restore_balanced_plan() -> Result<(), String> {
    power::set_active(power::BALANCED)
}

// ---------- Monitor ----------

#[tauri::command]
async fn perf_snapshot(state: State<'_, AppState>) -> Result<perf::Snapshot, String> {
    let mut p = state.perf.lock().unwrap_or_else(|e| e.into_inner());
    Ok(p.snapshot())
}

#[tauri::command]
async fn kill_process(state: State<'_, AppState>, pid: u32) -> Result<(), String> {
    let mut p = state.perf.lock().unwrap_or_else(|e| e.into_inner());
    p.kill(pid)
}

// ---------- RAM ----------

#[derive(Serialize)]
struct RamStatus {
    status: memory::MemStatus,
    detail: perf::MemDetail,
    processes: Vec<perf::ProcInfo>,
    admin: bool,
}

#[tauri::command]
async fn ram_status(state: State<'_, AppState>) -> Result<RamStatus, String> {
    let mut p = state.perf.lock().unwrap_or_else(|e| e.into_inner());
    Ok(RamStatus {
        status: memory::status(),
        detail: p.mem_detail(),
        processes: p.top_memory(20),
        admin: sysutil::is_admin(),
    })
}

#[tauri::command]
async fn ram_clean(options: Vec<String>) -> Result<memory::CleanResult, String> {
    Ok(memory::clean(&options))
}

#[tauri::command]
async fn ram_trim_process(pid: u32) -> Result<bool, String> {
    Ok(memory::trim_process(pid))
}

// ---------- Optimización ----------

#[tauri::command]
async fn tweaks_list(state: State<'_, AppState>) -> Result<Vec<tweaks::TweakInfo>, String> {
    let hw = get_hw(&state)?;
    Ok(tweaks::list(&hw))
}

#[tauri::command]
async fn tweak_set(state: State<'_, AppState>, id: String, enabled: bool) -> Result<tweaks::TweakInfo, String> {
    let hw = get_hw(&state)?;
    if enabled {
        tweaks::apply(&id, &hw)
    } else {
        tweaks::revert(&id, &hw)
    }
}

#[tauri::command]
async fn create_restore_point() -> Result<(), String> {
    tweaks::create_restore_point()
}

// ---------- Limpieza ----------

#[tauri::command]
async fn cleaner_scan() -> Result<Vec<cleaner::CleanItem>, String> {
    Ok(cleaner::scan())
}

#[tauri::command]
async fn cleaner_run(ids: Vec<String>) -> Result<cleaner::CleanReport, String> {
    Ok(cleaner::clean(&ids))
}

// ---------- Navegadores ----------

#[tauri::command]
async fn browsers_list() -> Result<Vec<browsers::BrowserInfo>, String> {
    Ok(browsers::list())
}

#[tauri::command]
async fn browser_clean(id: String) -> Result<u64, String> {
    browsers::clean(&id)
}

// ---------- Inicio de Windows ----------

#[tauri::command]
async fn startup_list() -> Result<Vec<startup::StartupEntry>, String> {
    Ok(startup::list())
}

#[tauri::command]
async fn startup_set(id: String, enabled: bool) -> Result<(), String> {
    startup::set_enabled(&id, enabled)
}

// ---------- Drivers ----------

#[tauri::command]
async fn drivers_info(state: State<'_, AppState>) -> Result<drivers::DriversInfo, String> {
    let hw = get_hw(&state)?;
    Ok(drivers::info(&hw))
}

// ---------- Herramientas ----------

#[tauri::command]
async fn tools_list() -> Result<Vec<tools::ToolInfo>, String> {
    tools::list()
}

#[tauri::command]
async fn tool_install(id: String) -> Result<(), String> {
    tools::install(&id)
}

#[tauri::command]
async fn tool_uninstall(id: String) -> Result<(), String> {
    tools::uninstall(&id)
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            scan: Mutex::new(None),
            progress: Arc::new(Progress::default()),
            scanning: AtomicBool::new(false),
            perf: Mutex::new(perf::Perf::new()),
            hw: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            relaunch_admin,
            open_target,
            hardware,
            list_drives,
            start_scan,
            cancel_scan,
            get_children,
            top_files,
            ext_stats,
            open_in_explorer,
            move_to_trash,
            get_power_plans,
            set_power_plan,
            enable_ultimate_plan,
            restore_balanced_plan,
            perf_snapshot,
            kill_process,
            ram_status,
            ram_clean,
            ram_trim_process,
            tweaks_list,
            tweak_set,
            create_restore_point,
            cleaner_scan,
            cleaner_run,
            browsers_list,
            browser_clean,
            startup_list,
            startup_set,
            drivers_info,
            tools_list,
            tool_install,
            tool_uninstall,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la aplicación");
}
