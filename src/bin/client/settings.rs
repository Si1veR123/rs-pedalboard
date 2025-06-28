use std::{process::Child, time::Instant, path::PathBuf};

use cpal::{Host, HostId};
use eframe::egui::{self, Color32, Layout, Response, RichText, Vec2, Widget};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use crate::state::State;
use crate::server_process::start_server_process;
use rs_pedalboard::{audio_devices::{get_input_devices, get_output_devices}, server_settings::{ServerSettingsSave, SupportedHost}, SAVE_DIR};

pub const CLIENT_SAVE_NAME: &'static str = "client_settings.json";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClientSettings {
    pub startup_server: bool,
    pub kill_server_on_close: bool
}

impl ClientSettings {
    fn get_save_path() -> Option<PathBuf> {
        Some(homedir::my_home().ok()??.join(SAVE_DIR).join(CLIENT_SAVE_NAME))
    }

    pub fn load() -> Result<Self, String> {
        match std::fs::read_to_string(Self::get_save_path().expect("Failed to get client settings save path")) {
            Ok(data) => serde_json::from_str(&data).map_err(|e| format!("Failed to deserialize client settings, error: {}", e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err("Client settings file not found".to_string())
            },
            Err(e) => Err(format!("Failed to read client settings file, error: {}", e)),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let data = serde_json::to_string(self).map_err(|e| format!("Failed to serialize client settings, error: {}", e))?;
        std::fs::write(Self::get_save_path().expect("Failed to get client settings save path"), data).map_err(|e| format!("Failed to write client settings file, error: {}", e))?;
        Ok(())
    }
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            startup_server: true,
            kill_server_on_close: true
        }
    }
}

pub enum ServerLaunchState {
    AwaitingKill(Instant),
    KillError,
    AwaitingStart {
        start_time: Instant,
        process: Child
    },
    StartError,
    None
}

impl ServerLaunchState {
    pub fn is_awaiting(&self) -> bool {
        matches!(self, ServerLaunchState::AwaitingKill(_) | ServerLaunchState::AwaitingStart { .. })
    }
}

struct AudioDevices {
    host_id: HostId,
    pub input_devices: Vec<String>,
    pub output_devices: Vec<String>,
    last_updated: Instant,
}

impl AudioDevices {
    fn new(host_id: HostId) -> Self {
        let host = Self::get_host(host_id);
        let input_devices = get_input_devices(host.as_ref()).unwrap_or_default();
        let output_devices = get_output_devices(host.as_ref()).unwrap_or_default();

        Self {
            host_id,
            input_devices,
            output_devices,
            last_updated: Instant::now(),
        }
    }

    fn get_host(host_id: HostId) -> Option<Host> {
        // JACK doesnt need host to get devices
        if cfg!(target_os = "linux") {
            None
        } else {
            Some(cpal::host_from_id(host_id).expect("Failed to get host from ID"))
        }
    }

    fn update(&mut self) {
        if self.last_updated.elapsed().as_secs() > 30 {
            let host = Self::get_host(self.host_id);
            self.input_devices = get_input_devices(host.as_ref()).unwrap_or_default();
            self.output_devices = get_output_devices(host.as_ref()).unwrap_or_default();
            self.last_updated = Instant::now();
        }
    }
}

pub struct SettingsScreen {
    program_state: &'static State,
    pub server_settings: ServerSettingsSave,
    pub client_settings: ClientSettings,
    pub server_launch_state: ServerLaunchState,
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

        let client_settings = match ClientSettings::load() {
            Ok(data) => data,
            Err(e) => {
                log::error!("{}", e);
                ClientSettings::default()
            }
        };

        Self {
            audio_devices: AudioDevices::new(server_settings.host.into()),
            program_state: state,
            server_settings,
            client_settings,
            server_launch_state: ServerLaunchState::None,
        }
    }

    #[cfg(target_os = "linux")]
    pub fn ready_to_start_server(&self) -> bool {
        self.server_settings.input_device.is_some() && self.server_settings.output_device.is_some() &&
        matches!(
            self.server_launch_state,
            ServerLaunchState::None | ServerLaunchState::StartError | ServerLaunchState::KillError
        )
    }

    #[cfg(target_os = "windows")]
    // On windows, we have the possibility of ASIO which only requires output device to be set
    pub fn ready_to_start_server(&self) -> bool {
        let correct_state = matches!(
            self.server_launch_state,
            ServerLaunchState::None | ServerLaunchState::StartError | ServerLaunchState::KillError
        );
        if self.server_settings.host == SupportedHost::Asio {
            self.server_settings.output_device.is_some() && correct_state
        } else {
            self.server_settings.input_device.is_some() && self.server_settings.output_device.is_some() && correct_state
        }
    }

    pub fn handle_server_launch(&mut self) {
        // Remove error state if now connected
        if self.program_state.socket.borrow().is_connected() {
            if matches!(self.server_launch_state, ServerLaunchState::KillError | ServerLaunchState::StartError) {
                self.server_launch_state = ServerLaunchState::None;
            }
        }

        if let ServerLaunchState::AwaitingKill(start_time) = self.server_launch_state {
            if start_time.elapsed().as_secs() > 5 {
                log::error!("Failed to stop server");
                self.server_launch_state = ServerLaunchState::KillError;
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
            // `try_wait` returns Ok(Some(status)) if the process has exited
            if start_time.elapsed().as_secs() > 5 || matches!(process.try_wait(), Ok(Some(_))) {
                log::error!("Server process started but did not connect, or closed. Check server logs");
                self.server_launch_state = ServerLaunchState::StartError;
            } else {
                if self.program_state.socket.borrow_mut().connect().is_ok() {
                    self.server_launch_state = ServerLaunchState::None;
                    log::info!("Server started successfully");

                    self.program_state.load_active_set();
                }
            }
        }
    }
}

impl Widget for &mut SettingsScreen {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        ui.add_space(ui.available_height()*0.05);
        ui.allocate_ui_with_layout(ui.available_size(), Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add_space(ui.available_width()*0.05);
            ui.allocate_ui_with_layout(ui.available_size()*Vec2::new(0.9, 0.9), Layout::top_down(egui::Align::Min), |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.style_mut().spacing.slider_width = ui.available_width()*0.4;

                    ui.label(RichText::new("Server Settings").font(egui::TextStyle::Heading.resolve(ui.style())));
                    ui.separator();

                    egui::Grid::new("server_settings_grid")
                        .num_columns(2)
                        .min_col_width(ui.available_width()*0.5)
                        .min_row_height(45.0)
                        .striped(true)
                        .show(ui, |ui| {
                            // Audio Host
                            // Only show if the platform has multiple host options
                            if SupportedHost::iter().count() > 1 {
                                ui.label("Host");
                                let prev_host = self.server_settings.host;
                                egui::ComboBox::new("host_dropdown", "")
                                    .selected_text(self.server_settings.host.to_string())
                                    .show_ui(ui, |ui| {
                                        for host in SupportedHost::iter() {
                                            ui.selectable_value(&mut self.server_settings.host, host, host.to_string());
                                        }
                                    });
                                // Selection has changed
                                if prev_host != self.server_settings.host {
                                    self.audio_devices = AudioDevices::new(self.server_settings.host.into());
                                    self.server_settings.input_device = None;
                                    self.server_settings.output_device = None;
                                }
                                ui.end_row();
                            }

                            // If on windows, and using ASIO host, we cannot control audio devices. Instead, we select the ASIO driver
                            #[cfg(target_os = "windows")]
                            let show_asio_driver = self.server_settings.host == SupportedHost::Asio;
                            #[cfg(not(target_os = "windows"))]
                            let show_asio_driver = false;

                            if show_asio_driver {
                                // ASIO Driver
                                ui.label("ASIO Driver");
                                if egui::ComboBox::from_id_salt("output_device_dropdown")
                                    .wrap_mode(egui::TextWrapMode::Truncate)
                                    .selected_text(self.server_settings.output_device.clone().unwrap_or_else(|| "None".to_string()))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.server_settings.output_device, None, "None");
                                        for device in &self.audio_devices.output_devices {
                                            ui.selectable_value(&mut self.server_settings.output_device, Some(device.clone()), device);
                                        }
                                }).response.clicked() {
                                    self.audio_devices.update();
                                };
                                ui.end_row();
                            } else {
                                // Input Devices
                                ui.label("Input Device");
                                if egui::ComboBox::from_id_salt("input_device_dropdown")
                                    .wrap_mode(egui::TextWrapMode::Truncate)
                                    .selected_text(self.server_settings.input_device.clone().unwrap_or_else(|| "None".to_string()))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.server_settings.input_device, None, "None");
                                        for device in &self.audio_devices.input_devices {
                                            ui.selectable_value(&mut self.server_settings.input_device, Some(device.clone()), device);
                                        }
                                }).response.clicked() {
                                    self.audio_devices.update();
                                };
                                ui.end_row();

                                // Output Devices
                                ui.label("Output Device");
                                if egui::ComboBox::from_id_salt("output_device_dropdown")
                                    .wrap_mode(egui::TextWrapMode::Truncate)
                                    .selected_text(self.server_settings.output_device.clone().unwrap_or_else(|| "None".to_string()))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.server_settings.output_device, None, "None");
                                        for device in &self.audio_devices.output_devices {
                                            ui.selectable_value(&mut self.server_settings.output_device, Some(device.clone()), device);
                                        }
                                }).response.clicked() {
                                    self.audio_devices.update();
                                };
                                ui.end_row();
                            }

                            // Buffer Size
                            let current_buffer_size = self.server_settings.buffer_size_samples();
                            ui.label(format!("Buffer Size - {} samples", current_buffer_size));
                            ui.add_sized(
                                Vec2::new(ui.available_width(), 45.0),
                                egui::Slider::new(&mut self.server_settings.buffer_size, 6..=12)
                                    .show_value(false)
                            );
                            ui.end_row();

                            // Latency
                            ui.label(format!("Latency - {:.2} ms", self.server_settings.latency));
                            ui.add_sized(
                                Vec2::new(ui.available_width(), 45.0),
                                egui::Slider::new(&mut self.server_settings.latency, 0.0..=25.0)
                                    .show_value(false)
                            );
                            ui.end_row();

                            // Periods per Buffer (JACK/Linux)
                            if cfg!(target_os = "linux") {
                                let periods_per_buffer = self.server_settings.periods_per_buffer;
                                ui.label(format!("Periods per Buffer - {periods_per_buffer}"));
                                ui.add_sized(
                                    Vec2::new(ui.available_width(), 45.0),
                                    egui::Slider::new(&mut self.server_settings.periods_per_buffer, 1..=4)
                                        .show_value(false)
                                );
                                ui.end_row();
                            };

                            // Tuner Periods
                            let tuner_periods = self.server_settings.tuner_periods;
                            ui.label(format!("Tuner Periods - {tuner_periods}"));
                            ui.add_sized(
                                Vec2::new(ui.available_width(), 45.0),
                                egui::Slider::new(&mut self.server_settings.tuner_periods, 1..=8)
                                    .show_value(false)
                            ).on_hover_text("Higher values may improve accuracy but increase computation, and decrease update time.");
                            ui.end_row();
                        });
                    
                    ui.add_space(10.0);
                    let button_size = Vec2::new(ui.available_width() * 0.25, 45.0);
                    ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), button_size.y), Layout::left_to_right(egui::Align::Center), |ui| {
                        let is_connected = self.program_state.socket.borrow().is_connected();
                        // If not connected, make spacing for 2 buttons. Else make spacing for 1.
                        let button_horizontal_space = if is_connected {
                            ui.available_width()/2.0-button_size.x/2.0
                        } else {
                            ui.available_width()/4.0-button_size.x/2.0
                        };
                        ui.add_space(button_horizontal_space);

                        let currently_connected = self.program_state.socket.borrow().is_connected();
                        let button_text = if currently_connected {
                            "Restart Server"
                        } else {
                            "Start Server"
                        };
                        if ui.add_enabled(
                            self.ready_to_start_server(), 
                            egui::Button::new(button_text)
                                .stroke(egui::Stroke::new(1.0, crate::THEME_COLOUR))
                                .min_size(button_size)
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

                        if !is_connected {
                            ui.add_space(button_horizontal_space*2.0);
                            let button = ui.add(
                                egui::Button::new("Connect")
                                    .stroke(egui::Stroke::new(1.0, crate::ROW_COLOUR_LIGHT))
                                    .min_size(button_size)
                            );

                            if button.clicked() {
                                log::info!("Connecting to server...");
                                match self.program_state.socket.borrow_mut().connect() {
                                    Ok(_) => {
                                        log::info!("Connected to server; Loading set...");
                                        self.program_state.load_active_set();
                                    },
                                    Err(e) => log::error!("Failed to connect to server: {}", e)
                                }
                            }
                        }
                    });
                    ui.add_space(15.0);
                    self.handle_server_launch();
                    match self.server_launch_state {
                        ServerLaunchState::StartError => { ui.label(RichText::new("Failed to start server. Check the logs for more details.").color(Color32::RED)); },
                        ServerLaunchState::KillError => { ui.label(RichText::new("Failed to stop server. Check the logs for more details.").color(Color32::RED)); },
                        ServerLaunchState::AwaitingKill(_) | ServerLaunchState::AwaitingStart { .. } => { ui.ctx().request_repaint(); }
                        ServerLaunchState::None => {}
                    }
                    ui.add_space(20.0);

                    ui.label(RichText::new("Client Settings").font(egui::TextStyle::Heading.resolve(ui.style())));
                    ui.separator();

                    egui::Grid::new("client_settings_grid")
                        .num_columns(2)
                        .min_col_width(ui.available_width()*0.5)
                        .min_row_height(45.0)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.style_mut().spacing.icon_width = 35.0;
                            ui.style_mut().spacing.icon_width_inner = 12.0;
                            ui.style_mut().visuals.widgets.inactive.fg_stroke = egui::Stroke::new(2.0, Color32::from_rgb(200, 200, 200));

                            ui.label("Startup Server");
                            ui.checkbox(&mut self.client_settings.startup_server, "");
                            ui.end_row();

                            ui.label("Kill Server on Close");
                            ui.checkbox(&mut self.client_settings.kill_server_on_close, "");
                        });
                })
            });
        }).response
    }
}