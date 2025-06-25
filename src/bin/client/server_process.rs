use std::{env::var, path::PathBuf, process::Child};
use rs_pedalboard::server_settings::ServerSettingsSave;
use which::which;

pub const SERVER_EXE_NAME: &str = "rs_pedalboard_server";
pub const SERVER_ENV_VAR: &str = "RSPEDALBOARD_SERVER";

pub fn get_server_executable_path() -> Option<PathBuf> {
    var(SERVER_ENV_VAR).ok().and_then(|path| {
        let path = PathBuf::from(path);
        if path.exists() && path.is_file() {
            Some(path)
        } else {
            log::warn!("Server executable path from environment variable {} does not exist: {}", SERVER_ENV_VAR, path.display());
            None
        }
    }).or_else(|| {
        which(SERVER_EXE_NAME).ok()
    })
}

pub fn start_server_process(settings: &ServerSettingsSave) -> Option<Child> {
    if settings.input_device.is_none() || settings.output_device.is_none() {
        log::error!("Input or output device is not set. Cannot start server.");
        return None;
    }

    match get_server_executable_path() {
        Some(path) => {
            let mut command = std::process::Command::new(path);
            let full_command = command.arg("--frames-per-period").arg(settings.buffer_size_samples().to_string())
                .arg("--periods-per-buffer").arg(settings.periods_per_buffer.to_string())
                .arg("--buffer-latency").arg(settings.latency.to_string())
                .arg("--tuner-periods").arg(settings.tuner_periods.to_string())
                .arg("--input-device").arg(settings.input_device.as_ref().unwrap())
                .arg("--output-device").arg(settings.output_device.as_ref().unwrap())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            log::info!("Full command to start server: {:?}", full_command);
            let process = full_command.spawn();

            match process {
                Ok(child) => {
                    log::info!("Server process started successfully with PID: {}", child.id());
                    Some(child)
                },
                Err(e) => {
                    log::error!("Failed to start server process: {}", e);
                    None
                }
            }
        },
        None => {
            log::error!("Server executable not found. Please set the {} environment variable or ensure the executable ({}) is in your PATH.", SERVER_ENV_VAR, SERVER_EXE_NAME);
            None
        }
    }
}
