use anyhow::Result;
use notify::{Watcher, RecursiveMode};
use std::path::Path;
use std::fs;
use anyhow::anyhow;

pub const KPM_DIR: &str = "/data/adb/kpm";
pub const KPMMGR_PATH: &str = "/data/adb/ksu/bin/kpmmgr";


pub fn ensure_kpm_dir() -> Result<()> {
    if !Path::new(KPM_DIR).exists() {
        fs::create_dir_all(KPM_DIR)?;
    }
    Ok(())
}

pub fn start_kpm_watcher() -> Result<()> {
    ensure_kpm_dir()?;
    load_existing_kpms()?;

    // 检查是否处于安全模式
    if crate::utils::is_safe_mode() {
        log::warn!("The system is in safe mode and is deleting all KPM modules...");
        if let Err(e) = remove_all_kpms() {
            log::error!("Error deleting all KPM modules: {}", e);
        }
        return Ok(());
    }

    let mut watcher = notify::recommended_watcher(|res| {
        match res {
            Ok(event) => handle_kpm_event(event),
            Err(e) => log::error!("monitoring error: {:?}", e),
        }
    })?;

    watcher.watch(Path::new(KPM_DIR), RecursiveMode::NonRecursive)?;
    Ok(())
}

pub fn handle_kpm_event(event: notify::Event) {
    match event.kind {
        notify::EventKind::Create(_) => {
            event.paths.iter().for_each(|path| {
                if path.extension().is_some_and(|ext| ext == "kpm") {
                    let _ = load_kpm(path);
                }
            });
        }
        notify::EventKind::Remove(_) => {
            event.paths.iter().for_each(|path| {
                if let Some(name) = path.file_stem() {
                    let _ = unload_kpm(name.to_string_lossy().as_ref());
                }
            });
        }
        _ => {}
    }
}

pub fn load_kpm(path: &Path) -> Result<()> {
    let status = std::process::Command::new(KPMMGR_PATH)
        .args(["load", path.to_str().unwrap(), ""])
        .status()?;
    
    if status.success() {
        log::info!("Loaded KPM: {}", path.display());
    }
    Ok(())
}

pub fn unload_kpm(name: &str) -> Result<()> {
    let status = std::process::Command::new(KPMMGR_PATH)
        .args(["unload", name])
        .status()
        .map_err(|e| anyhow!("Failed to execute kpmmgr: {}", e))?;

    let kpm_path = Path::new(KPM_DIR).join(format!("{}.kpm", name));
    if kpm_path.exists() {
        fs::remove_file(&kpm_path)
            .map_err(|e| anyhow!("Failed to delete KPM file: {}", e))
            .map(|_| log::info!("Deleted KPM file: {}", kpm_path.display()))?;
    }

    if status.success() {
        log::info!("Successfully unloaded KPM: {}", name);
    } else {
        log::warn!("KPM unloading may have failed: {}", name);
    }
    
    Ok(())
}

pub fn remove_all_kpms() -> Result<()> {
    ensure_kpm_dir()?;
    
    for entry in fs::read_dir(KPM_DIR)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "kpm") {
            if let Some(name) = path.file_stem() {
                unload_kpm(name.to_string_lossy().as_ref())
                    .unwrap_or_else(|e| log::error!("Failed to remove KPM: {}", e));
                let _ = fs::remove_file(&path);
            }
        }
    }
    Ok(())
}

// 加载所有现有的 KPM 模块
pub fn load_existing_kpms() -> Result<()> {
    ensure_kpm_dir()?;
    
    for entry in fs::read_dir(KPM_DIR)? {
        let path = entry?.path();
        if path.extension().map_or(false, |ext| ext == "kpm") {
            let _ = load_kpm(&path);
        }
    }
    Ok(())
}