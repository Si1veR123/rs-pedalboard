use std::{process::Child, time::Instant, path::PathBuf};

use cpal::{Host, HostId};
use eframe::egui::{self, Color32, Layout, Response, RichText, Vec2, Widget};
use rs_pedalboard::server_settings::ServerSettingsSave;
use serde::{Deserialize, Serialize};
use strum::{IntoEnumIterator};
use strum_macros::EnumIter;

use crate::state::State;
use crate::server_process::start_server_process;
use rs_pedalboard::{audio_devices::{get_input_devices, get_output_devices}, server_settings::{SupportedHost}, SAVE_DIR};

pub const CLIENT_SAVE_NAME: &'static str = "client_settings.json";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, EnumIter, PartialEq, Default)]
pub enum VolumeNormalizationMode {
    #[default]
    None,
    Manual,
    Automatic
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ClientSettings {
    pub startup_server: bool,
    pub kill_server_on_close: bool,
    pub show_volume_monitor: bool,
    pub volume_normalization: VolumeNormalizationMode,
    // Only used if volume_normalization is set to Automatic
    pub auto_volume_normalization_decay: f32,
    // Only used if volume_normalization is set to None
    pub input_volume: f32
}

impl ClientSettings {
    fn get_save_path() -> Option<PathBuf> {
        Some(homedir::my_home().ok()??.join(SAVE_DIR).join(CLIENT_SAVE_NAME))
    }

    pub fn load_or_default() -> Result<Self, std::io::Error> {
        let save_path = Self::get_save_path().expect("Failed to get client settings save path");

        if !save_path.exists() {
            log::info!("Client settings save file not found, using default");
            return Ok(Self::default());
        }

        let data = std::fs::read_to_string(save_path)?;

        Ok(serde_json::from_str(&data).expect("Failed to deserialize client settings"))
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let data = serde_json::to_string(self).expect("Failed to serialize client settings");
        std::fs::write(Self::get_save_path().expect("Failed to get client settings save path"), data)?;
        Ok(())
    }
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            startup_server: true,
            kill_server_on_close: true,
            show_volume_monitor: true,
            volume_normalization: VolumeNormalizationMode::None,
            auto_volume_normalization_decay: 0.95,
            input_volume: 1.0
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
    state: &'static State,
    pub server_launch_state: ServerLaunchState,
    audio_devices: AudioDevices,
}

impl SettingsScreen {
    pub fn new(state: &'static State) -> Self {
        Self {
            audio_devices: AudioDevices::new(state.server_settings.borrow().host.into()),
            state,
            server_launch_state: ServerLaunchState::None,
        }
    }

    #[cfg(target_os = "linux")]
    pub fn ready_to_start_server(&self, server_settings: &ServerSettingsSave) -> bool {
        server_settings.input_device.is_some() && server_settings.output_device.is_some() &&
        matches!(
            self.server_launch_state,
            ServerLaunchState::None | ServerLaunchState::StartError | ServerLaunchState::KillError
        )
    }

    #[cfg(target_os = "windows")]
    // On windows, we have the possibility of ASIO which only requires output device to be set
    pub fn ready_to_start_server(&self, server_settings: &ServerSettingsSave) -> bool {
        let correct_state = matches!(
            self.server_launch_state,
            ServerLaunchState::None | ServerLaunchState::StartError | ServerLaunchState::KillError
        );
        
        if server_settings.host == SupportedHost::ASIO {
            server_settings.output_device.is_some() && correct_state
        } else {
            server_settings.input_device.is_some() && server_settings.output_device.is_some() && correct_state
        }
    }

    /// Must be able to get a lock on socket and server_settings
    pub fn handle_server_launch(&mut self) {
        // Remove error state if now connected
        if self.state.is_connected() {
            if matches!(self.server_launch_state, ServerLaunchState::KillError | ServerLaunchState::StartError) {
                self.server_launch_state = ServerLaunchState::None;
            }
        }

        if let ServerLaunchState::AwaitingKill(start_time) = self.server_launch_state {
            if start_time.elapsed().as_secs() > 5 {
                log::error!("Failed to stop server");
                self.server_launch_state = ServerLaunchState::KillError;
            } else if start_time.elapsed().as_secs() > 1 {
                if !self.state.is_server_available() {
                    if let Some(process) = start_server_process(&self.state.server_settings.borrow()) {
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
                if self.state.connect_to_server().is_ok() {
                    self.server_launch_state = ServerLaunchState::None;
                    log::info!("Server started successfully");
                }
            }
        }
    }
}

impl Widget for &mut SettingsScreen {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        self.handle_server_launch();

        let mut server_settings = self.state.server_settings.borrow_mut();
        let mut client_settings = self.state.client_settings.borrow_mut();

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
                                let prev_host = server_settings.host;
                                egui::ComboBox::new("host_dropdown", "")
                                    .selected_text(server_settings.host.to_string())
                                    .show_ui(ui, |ui| {
                                        for host in SupportedHost::iter() {
                                            ui.selectable_value(&mut server_settings.host, host, host.to_string());
                                        }
                                    });
                                // Selection has changed
                                if prev_host != server_settings.host {
                                    self.audio_devices = AudioDevices::new(server_settings.host.into());
                                    server_settings.input_device = None;
                                    server_settings.output_device = None;
                                }
                                ui.end_row();
                            }

                            // If on windows, and using ASIO host, we cannot control audio devices. Instead, we select the ASIO driver
                            #[cfg(target_os = "windows")]
                            let show_asio_driver = server_settings.host == SupportedHost::ASIO;
                            #[cfg(not(target_os = "windows"))]
                            let show_asio_driver = false;

                            if show_asio_driver {
                                // ASIO Driver
                                ui.label("ASIO Driver");
                                if egui::ComboBox::from_id_salt("output_device_dropdown")
                                    .wrap_mode(egui::TextWrapMode::Truncate)
                                    .selected_text(server_settings.output_device.clone().unwrap_or_else(|| "None".to_string()))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut server_settings.output_device, None, "None");
                                        for device in &self.audio_devices.output_devices {
                                            ui.selectable_value(&mut server_settings.output_device, Some(device.clone()), device);
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
                                    .selected_text(server_settings.input_device.clone().unwrap_or_else(|| "None".to_string()))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut server_settings.input_device, None, "None");
                                        for device in &self.audio_devices.input_devices {
                                            ui.selectable_value(&mut server_settings.input_device, Some(device.clone()), device);
                                        }
                                }).response.clicked() {
                                    self.audio_devices.update();
                                };
                                ui.end_row();

                                // Output Devices
                                ui.label("Output Device");
                                if egui::ComboBox::from_id_salt("output_device_dropdown")
                                    .wrap_mode(egui::TextWrapMode::Truncate)
                                    .selected_text(server_settings.output_device.clone().unwrap_or_else(|| "None".to_string()))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut server_settings.output_device, None, "None");
                                        for device in &self.audio_devices.output_devices {
                                            ui.selectable_value(&mut server_settings.output_device, Some(device.clone()), device);
                                        }
                                }).response.clicked() {
                                    self.audio_devices.update();
                                };
                                ui.end_row();
                            }

                            // Buffer Size
                            let current_buffer_size = server_settings.buffer_size_samples();
                            ui.label(format!("Buffer Size - {} samples", current_buffer_size));
                            ui.add_sized(
                                Vec2::new(ui.available_width(), 45.0),
                                egui::Slider::new(&mut server_settings.buffer_size, 6..=12)
                                    .show_value(false)
                            );
                            ui.end_row();

                            // Latency
                            ui.label(format!("Latency - {:.2} ms", server_settings.latency));
                            ui.add_sized(
                                Vec2::new(ui.available_width(), 45.0),
                                egui::Slider::new(&mut server_settings.latency, 0.0..=25.0)
                                    .show_value(false)
                            );
                            ui.end_row();

                            // Periods per Buffer (JACK/Linux)
                            if cfg!(target_os = "linux") {
                                let periods_per_buffer = server_settings.periods_per_buffer;
                                ui.label(format!("Periods per Buffer - {periods_per_buffer}"));
                                ui.add_sized(
                                    Vec2::new(ui.available_width(), 45.0),
                                    egui::Slider::new(&mut server_settings.periods_per_buffer, 1..=4)
                                        .show_value(false)
                                );
                                ui.end_row();
                            };

                            // Tuner Periods
                            let tuner_periods = server_settings.tuner_periods;
                            ui.label(format!("Tuner Periods - {tuner_periods}"));
                            ui.add_sized(
                                Vec2::new(ui.available_width(), 45.0),
                                egui::Slider::new(&mut server_settings.tuner_periods, 1..=8)
                                    .show_value(false)
                            ).on_hover_text("Higher values may improve accuracy but increase computation, and decrease update time.");
                            ui.end_row();

                            // Preferred Sample Rate
                            ui.label(format!("Preferred Sample Rate"));
                            egui::ComboBox::from_id_salt("preferred_sample_rate_dropdown")
                                .selected_text(match server_settings.preferred_sample_rate {
                                    Some(rate) => format!("{rate}hz"),
                                    None => "Default".to_string()
                                })
                                .wrap_mode(egui::TextWrapMode::Truncate)
                                .show_ui(ui, |ui| {
                                    let mut response = ui.selectable_value(&mut server_settings.preferred_sample_rate, None, "Default");
                                    response |= ui.selectable_value(&mut server_settings.preferred_sample_rate, Some(44100), "44100hz");
                                    response |= ui.selectable_value(&mut server_settings.preferred_sample_rate, Some(48000), "48000hz");
                                    response |= ui.selectable_value(&mut server_settings.preferred_sample_rate, Some(88200), "88200hz");
                                    response |= ui.selectable_value(&mut server_settings.preferred_sample_rate, Some(96000), "96000hz");
                                    response |= ui.selectable_value(&mut server_settings.preferred_sample_rate, Some(176400), "176400hz");
                                    response |= ui.selectable_value(&mut server_settings.preferred_sample_rate, Some(192000), "192000hz");
                                    response
                                });
                            ui.end_row()
                        });
                    
                    ui.add_space(10.0);
                    let button_size = Vec2::new(ui.available_width() * 0.25, 45.0);

                    // Connecting requires a lock on client settings so must be done after rendering settings
                    // Store the connect button response to use later
                    let mut connect_button: Option<Response> = None;

                    ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), button_size.y), Layout::left_to_right(egui::Align::Center), |ui| {
                        let is_connected = self.state.is_connected();
                        // If not connected, make spacing for 2 buttons. Else make spacing for 1.
                        let button_horizontal_space = if is_connected {
                            ui.available_width()/2.0-button_size.x/2.0
                        } else {
                            ui.available_width()/4.0-button_size.x/2.0
                        };
                        ui.add_space(button_horizontal_space);

                        let currently_connected = self.state.is_connected();
                        let button_text = if currently_connected {
                            "Restart Server"
                        } else {
                            "Start Server"
                        };
                        if ui.add_enabled(
                            self.ready_to_start_server(&server_settings), 
                            egui::Button::new(button_text)
                                .stroke(egui::Stroke::new(1.0, crate::THEME_COLOUR))
                                .min_size(button_size)
                        ).clicked() {
                            ui.ctx().request_repaint();
                            if currently_connected {
                                self.state.kill_server();
                                self.server_launch_state = ServerLaunchState::AwaitingKill(Instant::now());
                            } else {
                                if let Some(process) = start_server_process(&server_settings) {
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
                            connect_button = Some(ui.add(
                                egui::Button::new("Connect")
                                    .stroke(egui::Stroke::new(1.0, crate::ROW_COLOUR_LIGHT))
                                    .min_size(button_size)
                            ));
                        }
                    });
                    ui.add_space(15.0);

                    match self.server_launch_state {
                        ServerLaunchState::StartError => { ui.label(RichText::new("Failed to start server. Check the logs for more details.").color(Color32::RED)); },
                        ServerLaunchState::KillError => { ui.label(RichText::new("Failed to stop server. Check the logs for more details.").color(Color32::RED)); },
                        ServerLaunchState::AwaitingKill(_) | ServerLaunchState::AwaitingStart { .. } => { ui.ctx().request_repaint_after(rs_pedalboard::DEFAULT_REFRESH_DURATION); }
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
                            ui.label("Volume Normalization");
                            let mut normalization_mode_change = false;
                            egui::ComboBox::from_id_salt("volume_normalization_dropdown")
                                .selected_text(format!("{:?}", client_settings.volume_normalization))
                                .wrap_mode(egui::TextWrapMode::Truncate)
                                .show_ui(ui, |ui| {
                                    for value in VolumeNormalizationMode::iter() {
                                        let response = ui.selectable_value(&mut client_settings.volume_normalization, value.clone(), format!("{:?}", value));
                                        normalization_mode_change |= response.changed();
                                        if value == VolumeNormalizationMode::Automatic {
                                            response.on_hover_text("Automatically normalize volume based on the peak volume of the audio stream. The peak is decayed to adjust to decreases in input volume.");
                                        } else if value == VolumeNormalizationMode::Manual {
                                            response.on_hover_text("Volume is normalized using the peak of the input audio stream. If input volume is decreased, the peak must be manually reset.");
                                        }
                                    }
                                });

                            if normalization_mode_change {
                                self.state.set_volume_normalization_server(client_settings.volume_normalization, client_settings.auto_volume_normalization_decay);
                            };
                            ui.end_row();

                            if client_settings.volume_normalization == VolumeNormalizationMode::Automatic {
                                ui.label("Volume Normalization Decay");
                                if ui.add_sized(
                                    Vec2::new(ui.available_width(), 45.0),
                                    egui::Slider::new(&mut client_settings.auto_volume_normalization_decay, 0.9..=1.0)
                                        .show_value(true)
                                        .fixed_decimals(3)
                                ).on_hover_text("The decay of the peak per second. Lower values respond to decreases in volume quicker but cause more overall fluctuations. 1.0 = Manual.").changed() {
                                    self.state.set_volume_normalization_server(client_settings.volume_normalization, client_settings.auto_volume_normalization_decay);
                                };
                                ui.end_row();
                            }

                            match client_settings.volume_normalization {
                                VolumeNormalizationMode::None => {
                                    ui.label("Input Volume");
                                    if ui.add_sized(
                                        Vec2::new(ui.available_width(), 45.0),
                                        egui::Slider::new(&mut client_settings.input_volume, 0.1..=5.0)
                                            .show_value(true)
                                            .fixed_decimals(2)
                                    ).changed() {
                                        self.state.master_in_server(client_settings.input_volume);
                                    };
                                    ui.end_row();
                                },
                                VolumeNormalizationMode::Manual | VolumeNormalizationMode::Automatic => {
                                    // Show peak reset button
                                    ui.label("Reset Volume Normalization");

                                    if ui.add_sized(
                                        Vec2::new(ui.available_width()*0.5, ui.available_height()*0.75),
                                        egui::Button::new("Reset Peak")
                                    ).on_hover_text("Reset the current peak used to normalize volume.").clicked() {
                                        self.state.reset_volume_normalization_peak();
                                    };
                                    ui.end_row();
                                }
                            };

                            ui.style_mut().spacing.icon_width = 35.0;
                            ui.style_mut().spacing.icon_width_inner = 12.0;
                            ui.style_mut().visuals.widgets.inactive.fg_stroke = egui::Stroke::new(2.0, Color32::from_rgb(200, 200, 200));

                            ui.label("Startup Server");
                            ui.checkbox(&mut client_settings.startup_server, "");
                            ui.end_row();

                            ui.label("Kill Server on Close");
                            ui.checkbox(&mut client_settings.kill_server_on_close, "");
                            ui.end_row();

                            ui.label("Show Volume Monitor");
                            let volume_monitor_message = "This can affect performance as the UI will have to frequently update";
                            if ui.checkbox(&mut client_settings.show_volume_monitor, "").on_hover_text(volume_monitor_message).changed() {
                                self.state.set_volume_monitor_active_server(client_settings.show_volume_monitor);
                            }
                            ui.end_row();
                        });

                    if connect_button.is_some_and(|r| r.clicked()) {
                        drop(client_settings);
                        let _ = self.state.connect_to_server();
                    }
                })
            });
        }).response
    }
}