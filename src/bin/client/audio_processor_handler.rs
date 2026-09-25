use eframe::egui;
use rs_pedalboard::processor_settings::ProcessorSettingsSave;
use std::{env::var, path::PathBuf, process::Child};
use which::which;
use crate::popup_message::popup;

pub const PROCESSOR_EXE_NAME: &str = "pedalboard-processor";
pub const PROCESSOR_ENV_VAR: &str = "RSPEDALBOARD_PROCESSOR";

pub fn get_processor_executable_path() -> Option<PathBuf> {
    var(PROCESSOR_ENV_VAR)
        .ok()
        .and_then(|path| {
            let path = PathBuf::from(path);
            if path.exists() && path.is_file() {
                Some(path)
            } else {
                tracing::warn!(
                    "Processor executable path from environment variable {} does not exist: {}",
                    PROCESSOR_ENV_VAR,
                    path.display()
                );
                None
            }
        })
        .or_else(|| which(PROCESSOR_EXE_NAME).ok())
}

pub enum StartProcessorResult {
    Started(Child),
    FailedToStart,
    ExecutableNotFound,
}

pub fn start_processor_process(settings: &ProcessorSettingsSave, egui_ctx: Option<egui::Context>) -> StartProcessorResult {
    match get_processor_executable_path() {
        Some(path) => {
            let mut command = std::process::Command::new(path);
            let full_command = command
                .arg("--frames-per-period")
                .arg(settings.buffer_size_samples().to_string())
                .arg("--host")
                .arg(settings.host.to_string())
                .arg("--periods-per-buffer")
                .arg(settings.periods_per_buffer.to_string())
                .arg("--buffer-latency")
                .arg(settings.latency.to_string())
                .arg("--tuner-periods")
                .arg(settings.tuner_periods.to_string())
                .arg("--upsample-passes")
                .arg(settings.upsample_passes.to_string())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());

            // These may not necessarily be required, e.g. ASIO only uses input device
            if let Some(input_device) = &settings.input_device {
                full_command.arg("--input-device").arg(input_device);
            }
            if let Some(output_device) = &settings.output_device {
                full_command.arg("--output-device").arg(output_device);
            }
            if let Some(preferred_sample_rate) = settings.preferred_sample_rate {
                full_command
                    .arg("--preferred-sample-rate")
                    .arg(preferred_sample_rate.to_string());
            }

            tracing::debug!("Full command to start processor: {:?}", full_command);
            let process = full_command.spawn();

            match process {
                Ok(child) => {
                    tracing::info!(
                        "Processor process started successfully with PID: {}",
                        child.id()
                    );
                    StartProcessorResult::Started(child)
                }
                Err(e) => {
                    if let Some(ctx) = egui_ctx {
                        popup!(
                            ctx,
                            "Failed to start processor process. Check logs.",
                            Some("processor-start-error"),
                            crate::popup_message::PopupMessageType::Error
                        );
                    }
                    tracing::error!("Failed to start processor process: {}", e);
                    StartProcessorResult::FailedToStart
                }
            }
        }
        None => {
            tracing::warn!("Processor executable not found. Please set the {} environment variable or ensure the executable ({}) is in your PATH.", PROCESSOR_ENV_VAR, PROCESSOR_EXE_NAME);
            if let Some(ctx) = egui_ctx {
                popup!(
                    ctx,
                    "Processor executable not found.",
                    Some("processor-executable-not-found"),
                    crate::popup_message::PopupMessageType::Warning
                );
            }
            StartProcessorResult::ExecutableNotFound
        }
    }
}
