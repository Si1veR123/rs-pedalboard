use std::path::{Path, PathBuf};
use rs_pedalboard::{
    processor_settings::SAVE_NAME as PROCESSOR_SETTINGS_SAVE_NAME, processor_settings::RECORDINGS_SAVE_DIR,
    pedals::{NAM_SAVE_PATH, IR_SAVE_PATH}
};
use crate::{settings::CLIENT_SAVE_NAME, saved_pedalboards::SAVE_NAME as PEDALBOARDS_SAVE_NAME};


// https://stackoverflow.com/a/65192210
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub fn clear_directory_contents<P: AsRef<Path>>(dir: P) -> Result<(), String> {
    let dir_path = dir.as_ref();
    if !dir_path.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir_path).map_err(|e| format!("Failed to read directory {}: {}", dir_path.display(), e))? {
        let entry = entry.map_err(|e| format!("Failed to read directory entry in {}: {}", dir_path.display(), e))?;
        let path = entry.path();
        if path.is_dir() {
            std::fs::remove_dir_all(&path).map_err(|e| format!("Failed to remove directory {}: {}", path.display(), e))?;
        } else {
            std::fs::remove_file(&path).map_err(|e| format!("Failed to remove file {}: {}", path.display(), e))?;
        }
    }
    Ok(())
}

/// Copy the contents of a directory to a sibling backup directory with the same name and "_backup"
pub fn create_directory_backup<P: AsRef<Path>>(dir: P) -> Result<PathBuf, String> {
    let dir_path = dir.as_ref();
    let backup_dir = dir_path.with_file_name(format!("{}_backup", dir_path.file_name().unwrap().to_string_lossy()));

    if backup_dir.exists() {
        std::fs::remove_dir_all(&backup_dir).map_err(|e| format!("Failed to remove existing backup directory {}: {}", backup_dir.display(), e))?;
    }

    std::fs::create_dir_all(&backup_dir).map_err(|e| format!("Failed to create backup directory {}: {}", backup_dir.display(), e))?;
    copy_dir_all(&dir_path, &backup_dir).map_err(|e| format!("Failed to copy directory {} to backup {}: {}", dir_path.display(), backup_dir.display(), e))?;

    Ok(backup_dir)
}

pub fn restore_directory_backup<P: AsRef<Path>>(dir: P) -> Result<(), String> {
    let dir_path = dir.as_ref();
    let backup_dir = dir_path.with_file_name(format!("{}_backup", dir_path.file_name().unwrap().to_string_lossy()));

    if !backup_dir.exists() {
        return Err(format!("Backup directory {} does not exist", backup_dir.display()));
    }

    clear_directory_contents(&dir_path)?;
    copy_dir_all(&backup_dir, &dir_path).map_err(|e| format!("Failed to restore directory {} from backup {}: {}", dir_path.display(), backup_dir.display(), e))?;

    Ok(())
}

/// Restore files from the backup directory, only for files that are missing in the original directory.
fn restore_remaining_files_from_backup<P: AsRef<Path>>(dir: P) -> Result<(), String> {
    let dir_path = dir.as_ref();
    let backup_dir = dir_path.with_file_name(format!("{}_backup", dir_path.file_name().unwrap().to_string_lossy()));

    if !backup_dir.exists() {
        return Err(format!("Backup directory {} does not exist", backup_dir.display()));
    }

    for entry in std::fs::read_dir(&backup_dir).map_err(|e| format!("Failed to read backup directory {}: {}", backup_dir.display(), e))? {
        let entry = entry.map_err(|e| format!("Failed to read directory entry in {}: {}", backup_dir.display(), e))?;
        let path = entry.path();
        let relative_path = path.strip_prefix(&backup_dir).map_err(|e| format!("Failed to get relative path for {}: {}", path.display(), e))?;
        let original_path = dir_path.join(relative_path);

        if !original_path.exists() {
            if path.is_dir() {
                std::fs::create_dir_all(&original_path).map_err(|e| format!("Failed to create directory {}: {}", original_path.display(), e))?;
                copy_dir_all(&path, &original_path).map_err(|e| format!("Failed to copy directory {} to {}: {}", path.display(), original_path.display(), e))?;
            } else {
                std::fs::copy(&path, &original_path).map_err(|e| format!("Failed to copy file {} to {}: {}", path.display(), original_path.display(), e))?;
            }
        }
    }

    Ok(())
}

pub fn partial_restore_directory_backup<P: AsRef<Path>>(dir: P, sync_settings: &super::GithubSyncSettings) -> Result<(), String> {
    let dir_path = dir.as_ref();
    let backup_dir = dir_path.with_file_name(format!("{}_backup", dir_path.file_name().unwrap().to_string_lossy()));

    if !backup_dir.exists() {
        return Err(format!("Backup directory {} does not exist", backup_dir.display()));
    }

    // For each settings file/directory, check if it should be restored based on the sync settings, then copy.
    // Backups are also restored if files/directories are missing after the sync.

    let client_settings_path = dir_path.join(CLIENT_SAVE_NAME);
    if !sync_settings.sync_client_settings || !client_settings_path.exists() {
        let backup_client_settings_path = backup_dir.join(CLIENT_SAVE_NAME);
        if backup_client_settings_path.exists() {
            std::fs::copy(&backup_client_settings_path, &client_settings_path)
                .map_err(|e| format!("Failed to restore client settings from backup: {}", e))?;
        }
    }

    let processor_settings_path = dir_path.join(PROCESSOR_SETTINGS_SAVE_NAME);
    if !sync_settings.sync_processor_settings || !processor_settings_path.exists() {
        let backup_processor_settings_path = backup_dir.join(PROCESSOR_SETTINGS_SAVE_NAME);
        if backup_processor_settings_path.exists() {
            std::fs::copy(&backup_processor_settings_path, &processor_settings_path)
                .map_err(|e| format!("Failed to restore processor settings from backup: {}", e))?;
        }
    }

    let pedalboards_path = dir_path.join(PEDALBOARDS_SAVE_NAME);
    if !sync_settings.sync_pedalboards || !pedalboards_path.exists() {
        let backup_pedalboards_path = backup_dir.join(PEDALBOARDS_SAVE_NAME);
        if backup_pedalboards_path.exists() {
            std::fs::copy(&backup_pedalboards_path, &pedalboards_path)
                .map_err(|e| format!("Failed to restore pedalboards from backup: {}", e))?;
        }
    }

    let nam_path = dir_path.join(NAM_SAVE_PATH);
    if !sync_settings.sync_nam_directory || !nam_path.exists() {
        let backup_nam_path = backup_dir.join(NAM_SAVE_PATH);
        if backup_nam_path.exists() {
            clear_directory_contents(&nam_path)?;
            copy_dir_all(&backup_nam_path, &nam_path)
                .map_err(|e| format!("Failed to restore NAM from backup: {}", e))?;
        }
    }

    let ir_path = dir_path.join(IR_SAVE_PATH);
    if !sync_settings.sync_ir_directory || !ir_path.exists() {
        let backup_ir_path = backup_dir.join(IR_SAVE_PATH);
        if backup_ir_path.exists() {
            clear_directory_contents(&ir_path)?;
            copy_dir_all(&backup_ir_path, &ir_path)
                .map_err(|e| format!("Failed to restore IR from backup: {}", e))?;
        }
    }

    let recordings_path = dir_path.join(RECORDINGS_SAVE_DIR);
    if !sync_settings.sync_recordings_directory || !recordings_path.exists() {
        let backup_recordings_path = backup_dir.join(RECORDINGS_SAVE_DIR);
        if backup_recordings_path.exists() {
            clear_directory_contents(&recordings_path)?;
            copy_dir_all(&backup_recordings_path, &recordings_path)
                .map_err(|e| format!("Failed to restore Recordings from backup: {}", e))?;
        }
    }

    restore_remaining_files_from_backup(dir)?;

    Ok(())
}
