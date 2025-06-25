use std::{process::Child, time::Instant};

use cpal::Host;
use eframe::egui::{self, Layout, Response, Vec2, Widget};
use serde::{Deserialize, Serialize};

use crate::state::State;
use crate::server_process::start_server_process;
use rs_pedalboard::{audio_devices::{get_input_devices, get_output_devices, get_host}, server_settings::ServerSettingsSave};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClientSettings {
    pub startup_server: bool,
    pub kill_server_on_close: bool
}

enum ServerLaunchState {
    AwaitingKill(Instant),
    AwaitingStart {
        start_time: Instant,
        process: Child
    },
    None
}

struct AudioDevices {
    host: Option<Host>,
    pub input_devices: Vec<String>,
    pub output_devices: Vec<String>,
    last_updated: Instant,
}

impl AudioDevices {
    fn new() -> Self {
        let host = get_host();
        let input_devices = get_input_devices(host.as_ref()).unwrap_or_default();
        let output_devices = get_output_devices(host.as_ref()).unwrap_or_default();

        Self {
            host,
            input_devices,
            output_devices,
            last_updated: Instant::now(),
        }
    }

    fn update(&mut self) {
        if self.last_updated.elapsed().as_secs() > 30 {
            self.input_devices = get_input_devices(self.host.as_ref()).unwrap_or_default();
            self.output_devices = get_output_devices(self.host.as_ref()).unwrap_or_default();
            self.last_updated = Instant::now();
        }
    }
}

pub struct SettingsScreen {
    program_state: &'static State,
    pub server_settings: ServerSettingsSave,
    pub client_settings: ClientSettings,
    server_launch_state: ServerLaunchState,
    audio_devices: AudioDevices,
}

impl SettingsScreen {
    pub fn new(state: &'static State) -> Self {
        let server_settings = match ServerSettingsSave::load() {
            Ok(data) => data,
            Err(e) => {
                log::error!("{}", e);
                ServerSettingsSave::default()
            }
        };

        let client_settings = ClientSettings {
            startup_server: true, // TEMP
            kill_server_on_close: true
        };

        Self {
            program_state: state,
            server_settings,
            client_settings,
            server_launch_state: ServerLaunchState::None,
            audio_devices: AudioDevices::new(),
        }
    }

    fn ready_to_start_server(&self) -> bool {
        self.server_settings.input_device.is_some() && self.server_settings.output_device.is_some() && matches!(self.server_launch_state, ServerLaunchState::None)
    }
}

impl Widget for &mut SettingsScreen {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        ui.add_space(ui.available_height()*0.05);
        ui.allocate_ui_with_layout(ui.available_size(), Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add_space(ui.available_width()*0.05);
            ui.allocate_ui_with_layout(ui.available_size()*Vec2::new(0.9, 0.9), Layout::top_down(egui::Align::Min), |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label("Server Settings");
                    ui.separator();

                    // Buffer Size
                    let current_buffer_size = 2_usize.pow(self.server_settings.buffer_size as u32);
                    ui.add_sized(
                        Vec2::new(ui.available_width(), 45.0),
                        egui::Slider::new(&mut self.server_settings.buffer_size, 6..=12)
                            .show_value(false)
                            .text(format!("Buffer Size - {current_buffer_size}"))
                    );

                    // Latency
                    let latency = self.server_settings.latency;
                    ui.add_sized(
                        Vec2::new(ui.available_width(), 45.0),
                        egui::Slider::new(&mut self.server_settings.latency, 0.0..=50.0)
                            .step_by(0.01)
                            .show_value(false)
                            .text(format!("Latency - {:.2} ms", latency))
                    );

                    // Periods per Buffer (JACK/Linux)
                    if cfg!(target_os = "linux") {
                        let periods_per_buffer = self.server_settings.periods_per_buffer;
                        ui.add_sized(
                            Vec2::new(ui.available_width(), 45.0),
                            egui::Slider::new(&mut self.server_settings.periods_per_buffer, 1..=4)
                                .show_value(false)
                                .text(format!("Periods per Buffer (JACK) - {periods_per_buffer}"))
                        );
                    };

                    // Tuner Periods
                    let tuner_periods = self.server_settings.tuner_periods;
                    ui.add_sized(
                        Vec2::new(ui.available_width(), 45.0),
                        egui::Slider::new(&mut self.server_settings.tuner_periods, 1..=8)
                            .show_value(false)
                            .text(format!("Tuner Periods - {tuner_periods}"))
                    ).on_hover_text("Higher values may improve accuracy but increase computation, and decrease update time.");

                    // Input Devices
                    ui.label("Input Device");
                    if egui::ComboBox::from_id_salt("input_device_dropdown")
                        .height(45.0)
                        .selected_text(self.server_settings.input_device.clone().unwrap_or_else(|| "None".to_string()))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.server_settings.input_device, None, "None");
                            for device in &self.audio_devices.input_devices {
                                ui.selectable_value(&mut self.server_settings.input_device, Some(device.clone()), device);
                            }
                        }).response.clicked() {
                            self.audio_devices.update();
                        };
                    
                    // Output Devices
                    ui.label("Output Device");
                    if egui::ComboBox::from_id_salt("output_device_dropdown")
                        .height(45.0)
                        .selected_text(self.server_settings.output_device.clone().unwrap_or_else(|| "None".to_string()))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.server_settings.output_device, None, "None");
                            for device in &self.audio_devices.output_devices {
                                ui.selectable_value(&mut self.server_settings.output_device, Some(device.clone()), device);
                            }
                        }).response.clicked() {
                            self.audio_devices.update();
                        };

                    let currently_connected = self.program_state.socket.borrow().is_connected();
                    let button_text = if currently_connected {
                        "Restart Server"
                    } else {
                        "Start Server"
                    };
                    if ui.add_enabled(
                        self.ready_to_start_server(), 
                        egui::Button::new(button_text)
                    ).clicked() {
                        ui.ctx().request_repaint();
                        if currently_connected {
                            self.program_state.socket.borrow_mut().kill();
                            self.server_launch_state = ServerLaunchState::AwaitingKill(Instant::now());
                        } else {
                            if let Some(process) = start_server_process(&self.server_settings) {
                                self.server_launch_state = ServerLaunchState::AwaitingStart {
                                    start_time: Instant::now(),
                                    process
                                };
                            } else {
                                log::error!("Failed to start server process");
                            }
                        }
                    };

                    if let ServerLaunchState::AwaitingKill(start_time) = self.server_launch_state {
                        ui.ctx().request_repaint();
                        if start_time.elapsed().as_secs() > 5 {
                            log::error!("Failed to stop server");
                            self.server_launch_state = ServerLaunchState::None;
                        } else if start_time.elapsed().as_secs() > 1 {
                            if !self.program_state.socket.borrow_mut().is_server_available() {
                                if let Some(process) = start_server_process(&self.server_settings) {
                                    self.server_launch_state = ServerLaunchState::AwaitingStart {
                                        start_time: Instant::now(),
                                        process
                                    };
                                } else {
                                    log::error!("Failed to start server process");
                                    self.server_launch_state = ServerLaunchState::None;
                                }
                            }
                        }
                    } else if let ServerLaunchState::AwaitingStart { start_time, process  } = &mut self.server_launch_state {
                        ui.ctx().request_repaint();
                        // `try_wait` returns Ok(Some(status)) if the process has exited
                        if start_time.elapsed().as_secs() > 5 || matches!(process.try_wait(), Ok(Some(_))) {
                            log::error!("Failed to start server");
                            self.server_launch_state = ServerLaunchState::None;
                        } else {
                            if self.program_state.socket.borrow_mut().connect().is_ok() {
                                self.server_launch_state = ServerLaunchState::None;
                                log::info!("Server started successfully");

                                self.program_state.server_synchronise();
                            }
                        }
                    }

                    ui.label("Client Settings");
                    ui.separator();
                })
            });
        }).response
    }
}