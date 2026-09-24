use std::{path::PathBuf, process::Child, time::Instant};

use cpal::{Host, HostId};
use eframe::egui::{self, Color32, Layout, Response, RichText, Vec2, Widget};
use rs_pedalboard::processor_settings::ProcessorSettingsSave;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::audio_processor_handler::start_processor_process;
use crate::state::State;
use rs_pedalboard::{
    audio_devices::{get_input_devices, get_output_devices},
    pedals::{graphic_eq_editor_ui, EqPresets},
    processor_settings::SupportedHost,
    SAVE_DIR,
};

pub const CLIENT_SAVE_NAME: &'static str = "client_settings.json";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, EnumIter, PartialEq, Default)]
pub enum VolumeNormalizationMode {
    #[default]
    None,
    Manual,
    Automatic,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ClientSettings {
    pub startup_processor: bool,
    pub kill_processor_on_close: bool,
    pub show_volume_monitor: bool,
    pub volume_normalization: VolumeNormalizationMode,
    // Only used if volume_normalization is set to Automatic
    pub auto_volume_normalization_decay: f32,
    pub input_volume: f32,
    pub output_volume: f32,
    pub nam_folders: Vec<PathBuf>,
    pub ir_folders: Vec<PathBuf>,
    pub vst2_folders: Vec<PathBuf>,
    /// The EQs the user has created for the input, and the one that is applied to it
    pub global_input_eq: EqPresets,
    /// The EQs the user has created for the output, and the one that is applied to it
    ///
    /// EQs saved before the input and the output EQ were separate are saved under the key the
    /// output EQ used to have, so they are read as the EQs of this side of the signal chain
    #[serde(alias = "global_eq")]
    pub global_output_eq: EqPresets,
}

impl ClientSettings {
    fn get_save_path() -> Option<PathBuf> {
        Some(
            homedir::my_home()
                .ok()??
                .join(SAVE_DIR)
                .join(CLIENT_SAVE_NAME),
        )
    }

    pub fn load_or_default() -> Self {
        let save_path = match Self::get_save_path() {
            Some(path) => path,
            None => {
                tracing::error!("Failed to get client settings save path, using default");
                return Self::default();
            }
        };

        if !save_path.exists() {
            tracing::info!("Client settings save file not found, using default");
            return Self::default();
        }

        match std::fs::read_to_string(&save_path) {
            Ok(data) => match serde_json::from_str::<Self>(&data) {
                Ok(state) => state,
                Err(e) => {
                    tracing::error!(
                        "Failed to deserialize client settings from {:?}: {e}, using default",
                        save_path
                    );
                    Self::default()
                }
            },
            Err(e) => {
                tracing::error!(
                    "Failed to read client settings from {:?}: {e}, using default",
                    save_path
                );
                Self::default()
            }
        }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let data = serde_json::to_string(self).expect("Failed to serialize client settings");
        std::fs::write(
            Self::get_save_path().expect("Failed to get client settings save path"),
            data,
        )?;
        Ok(())
    }
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            startup_processor: true,
            kill_processor_on_close: true,
            show_volume_monitor: true,
            volume_normalization: VolumeNormalizationMode::None,
            auto_volume_normalization_decay: 0.95,
            input_volume: 1.0,
            output_volume: 1.0,
            nam_folders: vec![],
            ir_folders: vec![],
            vst2_folders: vec![],
            global_input_eq: EqPresets::default(),
            global_output_eq: EqPresets::default(),
        }
    }
}

pub enum ProcessorLaunchState {
    AwaitingKill(Instant),
    KillError,
    AwaitingStart { start_time: Instant, process: Child },
    StartError,
    None,
}

impl ProcessorLaunchState {
    pub fn is_awaiting(&self) -> bool {
        matches!(
            self,
            ProcessorLaunchState::AwaitingKill(_) | ProcessorLaunchState::AwaitingStart { .. }
        )
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
    pub processor_launch_state: ProcessorLaunchState,
    audio_devices: AudioDevices,

    input_eq_editor: EqEditorState,
    output_eq_editor: EqEditorState,
    /// Identifies the input EQ editor's widgets: the graph the editor draws is named under it. The
    /// input and the output editor are drawn in the same frame, so each is named with an ID of its
    /// own. A global EQ belongs to no pedal, so these are IDs no pedal has.
    input_eq_editor_id: u32,
    output_eq_editor_id: u32,

    nam_file_dialog: egui_file::FileDialog,
    ir_file_dialog: egui_file::FileDialog,
    vst2_file_dialog: egui_file::FileDialog,
}

/// The parts of one of the Global EQ sections that are not saved with the EQs themselves
#[derive(Default)]
struct EqEditorState {
    /// The open name field, if any: a new EQ being named, or one being renamed
    name_input: Option<EqNameInput>,
}

/// A name field for creating or renaming an EQ
struct EqNameInput {
    /// Index of the EQ being renamed, or `None` when the EQ does not exist yet
    rename: Option<usize>,
    /// The name being typed
    name: String,
    /// Whether the field has been given the keyboard yet, as it takes it when it opens
    focused: bool,
}

impl EqNameInput {
    /// A name field holding `name`, renaming the EQ at `rename`, or creating a new EQ if `None`
    fn new(rename: Option<usize>, name: String) -> Self {
        Self {
            rename,
            name,
            focused: false,
        }
    }
}

impl SettingsScreen {
    pub fn new(state: &'static State) -> Self {
        Self {
            audio_devices: AudioDevices::new(state.processor_settings.borrow().host.into()),
            state,
            processor_launch_state: ProcessorLaunchState::None,
            input_eq_editor: EqEditorState::default(),
            output_eq_editor: EqEditorState::default(),
            // A global EQ belongs to no pedal, so its editors are named with IDs no pedal has. The
            // two editors are drawn in the same frame, so each is named with an ID of its own.
            input_eq_editor_id: rs_pedalboard::unique_time_id(),
            output_eq_editor_id: rs_pedalboard::unique_time_id(),
            nam_file_dialog: egui_file::FileDialog::select_folder(),
            ir_file_dialog: egui_file::FileDialog::select_folder(),
            vst2_file_dialog: egui_file::FileDialog::select_folder(),
        }
    }

    #[cfg(target_os = "linux")]
    pub fn ready_to_start_processor(&self, processor_settings: &ProcessorSettingsSave) -> bool {
        processor_settings.input_device.is_some()
            && processor_settings.output_device.is_some()
            && matches!(
                self.processor_launch_state,
                ProcessorLaunchState::None
                    | ProcessorLaunchState::StartError
                    | ProcessorLaunchState::KillError
            )
    }

    #[cfg(target_os = "windows")]
    // On windows, we have the possibility of ASIO which only requires output device to be set
    pub fn ready_to_start_processor(&self, processor_settings: &ProcessorSettingsSave) -> bool {
        let correct_state = matches!(
            self.processor_launch_state,
            ProcessorLaunchState::None
                | ProcessorLaunchState::StartError
                | ProcessorLaunchState::KillError
        );

        if processor_settings.host == SupportedHost::ASIO {
            processor_settings.output_device.is_some() && correct_state
        } else {
            processor_settings.input_device.is_some()
                && processor_settings.output_device.is_some()
                && correct_state
        }
    }

    /// Must be able to get a lock on socket and processor_settings
    pub fn handle_processor_launch(&mut self) {
        // Remove error state if now connected
        if self.state.is_connected() {
            if matches!(
                self.processor_launch_state,
                ProcessorLaunchState::KillError | ProcessorLaunchState::StartError
            ) {
                self.processor_launch_state = ProcessorLaunchState::None;
            }
        }

        if let ProcessorLaunchState::AwaitingKill(start_time) = self.processor_launch_state {
            if start_time.elapsed().as_secs() > 5 {
                tracing::error!("Failed to stop processor");
                self.processor_launch_state = ProcessorLaunchState::KillError;
            } else if start_time.elapsed().as_secs() > 1 {
                if !self.state.is_processor_available() {
                    if let Some(process) =
                        start_processor_process(&self.state.processor_settings.borrow())
                    {
                        self.processor_launch_state = ProcessorLaunchState::AwaitingStart {
                            start_time: Instant::now(),
                            process,
                        };
                    } else {
                        tracing::error!("Failed to start processor process");
                        self.processor_launch_state = ProcessorLaunchState::None;
                    }
                }
            }
        } else if let ProcessorLaunchState::AwaitingStart {
            start_time,
            process,
        } = &mut self.processor_launch_state
        {
            // `try_wait` returns Ok(Some(status)) if the process has exited
            if start_time.elapsed().as_secs() > 5 || matches!(process.try_wait(), Ok(Some(_))) {
                tracing::error!("Processor process started but did not connect, or closed. Check processor logs");
                self.processor_launch_state = ProcessorLaunchState::StartError;
            } else {
                if self.state.connect_to_processor().is_ok() {
                    self.processor_launch_state = ProcessorLaunchState::None;
                    tracing::info!("Processor started successfully");
                }
            }
        }
    }
}

const SETTING_ROW_HEIGHT_FRACT: f32 = 0.1;
const BUTTON_EXPANSION: f32 = 2.0;
const SECTION_SPACE: f32 = 40.0;
/// The size a subheading under a section heading is written in, as a share of the size a heading is
/// written in, so that a subheading reads as being under its heading however large the font the
/// window is drawn with is.
const SUBHEADING_SIZE_FRACTION: f32 = 0.75;

impl Widget for &mut SettingsScreen {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        self.handle_processor_launch();

        let mut processor_settings = self.state.processor_settings.borrow_mut();
        let mut client_settings = self.state.client_settings.borrow_mut();

        ui.allocate_ui_with_layout(ui.available_size(), Layout::left_to_right(egui::Align::Min), |ui| {
            ui.add_space(ui.available_width()*0.05);
            ui.allocate_ui_with_layout(ui.available_size()*Vec2::new(0.95, 1.0), Layout::top_down(egui::Align::Min), |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.style_mut().visuals.widgets.open.expansion = BUTTON_EXPANSION;
                    ui.style_mut().visuals.widgets.inactive.expansion = BUTTON_EXPANSION;
                    ui.style_mut().visuals.widgets.hovered.expansion = BUTTON_EXPANSION;
                    ui.style_mut().visuals.widgets.active.expansion = BUTTON_EXPANSION;

                    ui.style_mut().spacing.slider_width = ui.available_width()*0.45 - 80.0;

                    ui.add_space(SECTION_SPACE);
                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui
                            .add_sized(Vec2::new(180.0, 55.0), egui::Button::new("Exit"))
                            .clicked()
                        {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });

                    ui.label(RichText::new("Processor Settings").font(egui::TextStyle::Heading.resolve(ui.style())));
                    ui.separator();

                    egui::Grid::new("processor_settings_grid")
                        .num_columns(2)
                        .min_col_width(ui.available_width()*0.5)
                        .min_row_height(SETTING_ROW_HEIGHT_FRACT * ui.ctx().viewport_rect().height())
                        .striped(true)
                        .show(ui, |ui| {
                            // Audio Host
                            // Only show if the platform has multiple host options
                            if SupportedHost::iter().count() > 1 {
                                ui.label("\tHost");
                                let prev_host = processor_settings.host;
                                egui::ComboBox::new("host_dropdown", "")
                                    .selected_text(processor_settings.host.to_string())
                                    .show_ui(ui, |ui| {
                                        for host in SupportedHost::iter() {
                                            ui.selectable_value(&mut processor_settings.host, host, host.to_string());
                                        }
                                    });
                                // Selection has changed
                                if prev_host != processor_settings.host {
                                    self.audio_devices = AudioDevices::new(processor_settings.host.into());
                                    processor_settings.input_device = None;
                                    processor_settings.output_device = None;
                                }
                                ui.end_row();
                            }

                            // If on windows, and using ASIO host, we cannot control audio devices. Instead, we select the ASIO driver
                            #[cfg(target_os = "windows")]
                            let show_asio_driver = processor_settings.host == SupportedHost::ASIO;
                            #[cfg(not(target_os = "windows"))]
                            let show_asio_driver = false;

                            if show_asio_driver {
                                // ASIO Driver
                                ui.label("\tASIO Driver");
                                if egui::ComboBox::from_id_salt("output_device_dropdown")
                                    .wrap_mode(egui::TextWrapMode::Truncate)
                                    .selected_text(processor_settings.output_device.clone().unwrap_or_else(|| "None".to_string()))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut processor_settings.output_device, None, "None");
                                        for device in &self.audio_devices.output_devices {
                                            ui.selectable_value(&mut processor_settings.output_device, Some(device.clone()), device);
                                        }
                                }).response.clicked() {
                                    self.audio_devices.update();
                                };
                                ui.end_row();
                            } else {
                                // Input Devices
                                ui.label("\tInput Device");
                                if egui::ComboBox::from_id_salt("input_device_dropdown")
                                    .wrap_mode(egui::TextWrapMode::Truncate)
                                    .selected_text(processor_settings.input_device.clone().unwrap_or_else(|| "None".to_string()))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut processor_settings.input_device, None, "None");
                                        for device in &self.audio_devices.input_devices {
                                            ui.selectable_value(&mut processor_settings.input_device, Some(device.clone()), device);
                                        }
                                }).response.clicked() {
                                    self.audio_devices.update();
                                };
                                ui.end_row();

                                // Output Devices
                                ui.label("\tOutput Device");
                                if egui::ComboBox::from_id_salt("output_device_dropdown")
                                    .wrap_mode(egui::TextWrapMode::Truncate)
                                    .selected_text(processor_settings.output_device.clone().unwrap_or_else(|| "None".to_string()))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut processor_settings.output_device, None, "None");
                                        for device in &self.audio_devices.output_devices {
                                            ui.selectable_value(&mut processor_settings.output_device, Some(device.clone()), device);
                                        }
                                }).response.clicked() {
                                    self.audio_devices.update();
                                };
                                ui.end_row();
                            }

                            // Buffer Size
                            ui.label("\tBuffer Size");
                            ui.add_sized(
                                Vec2::new(ui.available_width(), 45.0),
                                egui::Slider::new(&mut processor_settings.buffer_size, 6..=12)
                                    .custom_formatter(|value, _| format!("{}", 2_u32.pow(value as u32)))
                            );
                            ui.end_row();

                            // Latency
                            ui.label("\tLatency");
                            ui.add_sized(
                                Vec2::new(ui.available_width(), 45.0),
                                egui::Slider::new(&mut processor_settings.latency, 0.0..=25.0)
                                    .custom_formatter(|value, _| format!("{:.2}ms", value))
                            ).on_hover_text("Internal buffer latency. Increase latency if you experience X Runs (in this app)");
                            ui.end_row();

                            // Periods per Buffer (JACK/Linux)
                            if cfg!(target_os = "linux") {
                                ui.label("\tPeriods per Buffer");
                                ui.add_sized(
                                    Vec2::new(ui.available_width(), 45.0),
                                    egui::Slider::new(&mut processor_settings.periods_per_buffer, 1..=4)
                                );
                                ui.end_row();
                            };

                            // Tuner Periods
                            ui.label("\tTuner Periods");
                            ui.add_sized(
                                Vec2::new(ui.available_width(), 45.0),
                                egui::Slider::new(&mut processor_settings.tuner_periods, 1..=8)
                            ).on_hover_text("Higher values may improve accuracy but increase computation, and decrease update time.");
                            ui.end_row();

                            // Preferred Sample Rate
                            ui.label("\tPreferred Sample Rate");
                            egui::ComboBox::from_id_salt("preferred_sample_rate_dropdown")
                                .selected_text(match processor_settings.preferred_sample_rate {
                                    Some(rate) => format!("{rate}hz"),
                                    None => "Default".to_string()
                                })
                                .wrap_mode(egui::TextWrapMode::Truncate)
                                .show_ui(ui, |ui| {
                                    let mut response = ui.selectable_value(&mut processor_settings.preferred_sample_rate, None, "Default");
                                    response |= ui.selectable_value(&mut processor_settings.preferred_sample_rate, Some(44100), "44100hz");
                                    response |= ui.selectable_value(&mut processor_settings.preferred_sample_rate, Some(48000), "48000hz");
                                    response |= ui.selectable_value(&mut processor_settings.preferred_sample_rate, Some(88200), "88200hz");
                                    response |= ui.selectable_value(&mut processor_settings.preferred_sample_rate, Some(96000), "96000hz");
                                    response |= ui.selectable_value(&mut processor_settings.preferred_sample_rate, Some(176400), "176400hz");
                                    response |= ui.selectable_value(&mut processor_settings.preferred_sample_rate, Some(192000), "192000hz");
                                    response
                                });
                            ui.end_row();

                            // Upsample Passes
                            ui.label("\tUpsample");
                            egui::ComboBox::from_id_salt("upsample_dropdown")
                                .selected_text(match processor_settings.upsample_passes {
                                    0 => "None",
                                    1 => "2x",
                                    2 => "4x",
                                    3 => "8x",
                                    _ => "Error"
                                })
                                .wrap_mode(egui::TextWrapMode::Truncate)
                                .show_ui(ui, |ui| {
                                    let mut response = ui.selectable_value(&mut processor_settings.upsample_passes, 0, "None");
                                    response |= ui.selectable_value(&mut processor_settings.upsample_passes, 1, "2x");
                                    response |= ui.selectable_value(&mut processor_settings.upsample_passes, 2, "4x");
                                    response |= ui.selectable_value(&mut processor_settings.upsample_passes, 3, "8x");
                                    response
                                });

                            ui.end_row()
                        });

                    ui.add_space(20.0);
                    let button_size = Vec2::new(ui.available_width() * 0.25, ui.ctx().viewport_rect().height()*0.06);

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
                            "Restart Processor"
                        } else {
                            "Start Processor"
                        };
                        if ui.add_enabled(
                            self.ready_to_start_processor(&processor_settings),
                            egui::Button::new(button_text)
                                .stroke(egui::Stroke::new(1.0_f32, crate::THEME_COLOR))
                                .min_size(button_size)
                        ).clicked() {
                            ui.ctx().request_repaint();
                            if currently_connected {
                                self.state.kill_processor();
                                self.processor_launch_state = ProcessorLaunchState::AwaitingKill(Instant::now());
                            } else {
                                if let Some(process) = start_processor_process(&processor_settings) {
                                    self.processor_launch_state = ProcessorLaunchState::AwaitingStart {
                                        start_time: Instant::now(),
                                        process
                                    };
                                } else {
                                    tracing::error!("Failed to start processor process");
                                }
                            }
                        };

                        if !is_connected {
                            ui.add_space(button_horizontal_space*2.0);
                            connect_button = Some(ui.add(
                                egui::Button::new("Connect")
                                    .stroke(egui::Stroke::new(1.0_f32, crate::ROW_COLOR_LIGHT))
                                    .min_size(button_size)
                            ));
                        }
                    });

                    ui.add_space(15.0);

                    match self.processor_launch_state {
                        ProcessorLaunchState::StartError => { ui.label(RichText::new("Failed to start processor. Check the logs for more details.").color(Color32::RED)); },
                        ProcessorLaunchState::KillError => { ui.label(RichText::new("Failed to stop processor. Check the logs for more details.").color(Color32::RED)); },
                        ProcessorLaunchState::AwaitingKill(_) | ProcessorLaunchState::AwaitingStart { .. } => { ui.ctx().request_repaint_after(rs_pedalboard::DEFAULT_REFRESH_DURATION); }
                        ProcessorLaunchState::None => {}
                    }

                    ui.add_space(SECTION_SPACE);

                    ui.label(RichText::new("Client Settings").font(egui::TextStyle::Heading.resolve(ui.style())));
                    ui.separator();

                    egui::Grid::new("client_settings_grid")
                        .num_columns(2)
                        .min_col_width(ui.available_width()*0.5)
                        .min_row_height(SETTING_ROW_HEIGHT_FRACT * ui.ctx().viewport_rect().height())
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
                                self.state.set_volume_normalization_processor(client_settings.volume_normalization, client_settings.auto_volume_normalization_decay);
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
                                    self.state.set_volume_normalization_processor(client_settings.volume_normalization, client_settings.auto_volume_normalization_decay);
                                };
                                ui.end_row();
                            }

                            ui.label("Input Volume");
                            if ui.add_sized(
                                Vec2::new(ui.available_width(), 45.0),
                                egui::Slider::new(&mut client_settings.input_volume, 0.1..=5.0)
                                    .show_value(true)
                                    .fixed_decimals(2)
                            ).changed() {
                                self.state.master_in_processor(client_settings.input_volume);
                            };
                            ui.end_row();

                            ui.label("Output Volume");
                            if ui.add_sized(
                                Vec2::new(ui.available_width(), 45.0),
                                egui::Slider::new(&mut client_settings.output_volume, 0.01..=1.0)
                                    .show_value(true)
                                    .fixed_decimals(2)
                            ).changed() {
                                self.state.master_out_processor(client_settings.output_volume);
                            };
                            ui.end_row();

                            if matches!(client_settings.volume_normalization, VolumeNormalizationMode::Manual | VolumeNormalizationMode::Automatic) {
                                // Show peak reset button
                                ui.label("Reset Volume Normalization");

                                if ui.add_sized(
                                    Vec2::new(ui.available_width()*0.9, ui.available_height()*0.75),
                                    egui::Button::new("Reset Peak")
                                ).on_hover_text("Reset the current peak used to normalize volume.").clicked() {
                                    self.state.reset_volume_normalization_peak();
                                };
                                ui.end_row();
                            };

                            set_large_checkbox_style(ui);

                            ui.label("Startup Processor");
                            ui.checkbox(&mut client_settings.startup_processor, "");
                            ui.end_row();

                            ui.label("Kill Processor on Close");
                            ui.checkbox(&mut client_settings.kill_processor_on_close, "");
                            ui.end_row();

                            ui.label("Show Volume Monitor");
                            let volume_monitor_message = "This can affect performance as the UI will have to frequently update";
                            if ui.checkbox(&mut client_settings.show_volume_monitor, "").on_hover_text(volume_monitor_message).changed() {
                                self.state.set_volume_monitor_active_processor(client_settings.show_volume_monitor);
                            }
                            ui.end_row();
                        });

                    ui.add_space(SECTION_SPACE);

                    let input_eq_editor_id = self.input_eq_editor_id;
                    let output_eq_editor_id = self.output_eq_editor_id;

                    ui.heading("Global EQ");
                    ui.separator();

                    if global_eq_side_ui(
                        ui,
                        "Input EQ",
                        "Applied to the input, before the pedalboards are given it",
                        &mut self.input_eq_editor,
                        input_eq_editor_id,
                        &mut client_settings.global_input_eq,
                    ) {
                        self.state.set_input_global_eq_processor(
                            client_settings.global_input_eq.selected_eq().cloned(),
                        );
                    }

                    ui.add_space(SECTION_SPACE);

                    if global_eq_side_ui(
                        ui,
                        "Output EQ",
                        "Applied to the output, after the pedalboards have processed it",
                        &mut self.output_eq_editor,
                        output_eq_editor_id,
                        &mut client_settings.global_output_eq,
                    ) {
                        self.state.set_output_global_eq_processor(
                            client_settings.global_output_eq.selected_eq().cloned(),
                        );
                    }

                    ui.add_space(SECTION_SPACE);

                    ui.heading("Neural Amp Modeler Folders");
                    ui.separator();
                    ui.add_space(20.0);
                    if multiple_directories_select_ui(
                        ui,
                        &mut client_settings.nam_folders,
                        rs_pedalboard::pedals::Nam::get_save_directory(),
                        "nam_folders",
                        &mut self.nam_file_dialog
                    ) {
                        let nam_root_nodes: Vec<_> = client_settings.nam_folders.iter().map(|p| {
                            egui_directory_combobox::DirectoryNode::from_path(p)
                        }).collect();

                        ui.ctx().memory_mut(|writer| {
                            let nam_state = writer.data.get_temp_mut_or(egui::Id::new("nam_folders_state"), 1u32);
                            *nam_state += 1;
                            writer.data.insert_temp(egui::Id::new("nam_folders"), nam_root_nodes);
                        });
                    }

                    ui.add_space(SECTION_SPACE);

                    ui.heading("Impulse Response Folders");
                    ui.separator();
                    ui.add_space(20.0);
                    if multiple_directories_select_ui(
                        ui,
                        &mut client_settings.ir_folders,
                        rs_pedalboard::pedals::ImpulseResponse::get_save_directory(),
                        "ir_folders",
                        &mut self.ir_file_dialog
                    ) {
                        let ir_root_nodes: Vec<_> = client_settings.ir_folders.iter().map(|p| {
                            egui_directory_combobox::DirectoryNode::from_path(p)
                        }).collect();

                        ui.ctx().memory_mut(|writer| {
                            let ir_state = writer.data.get_temp_mut_or(egui::Id::new("ir_folders_state"), 1u32);
                            *ir_state += 1;
                            writer.data.insert_temp(egui::Id::new("ir_folders"), ir_root_nodes);
                        });
                    }

                    ui.add_space(SECTION_SPACE);

                    ui.heading("VST2 Plugin Folders");
                    ui.separator();
                    ui.add_space(20.0);
                    if multiple_directories_select_ui(
                        ui,
                        &mut client_settings.vst2_folders,
                        Some(PathBuf::from(rs_pedalboard::plugin::vst2::VST2_PLUGIN_PATH)),
                        "vst2_folders",
                        &mut self.vst2_file_dialog
                    ) {
                        let vst2_root_nodes: Vec<_> = client_settings.vst2_folders.iter().map(|p| {
                            egui_directory_combobox::DirectoryNode::from_path(p)
                        }).collect();

                        ui.ctx().memory_mut(|writer| {
                            let vst2_state = writer.data.get_temp_mut_or(egui::Id::new("vst2_folders_state"), 1u32);
                            *vst2_state += 1;
                            writer.data.insert_temp(egui::Id::new("vst2_folders"), vst2_root_nodes);
                        });
                    }

                    ui.add_space(SECTION_SPACE);

                    ui.heading("MIDI");
                    ui.separator();

                    self.state.midi_state.borrow_mut().midi_port_device_settings_ui(ui);

                    if connect_button.is_some_and(|r| r.clicked()) {
                        drop(client_settings);
                        let _ = self.state.connect_to_processor();
                    }
                })
            });
        }).response
    }
}

fn multiple_directories_select_ui(
    ui: &mut egui::Ui,
    paths: &mut Vec<PathBuf>,
    default_path: Option<PathBuf>,
    id: &str,
    file_dialog: &mut egui_file::FileDialog,
) -> bool {
    let mut changed = false;
    let available_width = ui.available_width();

    if ui
        .add_sized(
            Vec2::new(available_width * 0.3, 45.0),
            egui::Button::new("Add Directory"),
        )
        .clicked()
    {
        file_dialog.open();
    }

    file_dialog.show(ui.ctx());

    ui.add_space(10.0);

    if file_dialog.selected() {
        if let Some(path) = file_dialog.path() {
            let path = dunce::canonicalize(path);
            match path {
                Ok(path) => {
                    if path.is_dir() && !paths.contains(&path) {
                        paths.push(path);
                        changed = true;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to canonicalize path: {e}");
                    return false;
                }
            }
        }
    }

    egui::Grid::new(id)
        .num_columns(2)
        .min_row_height(SETTING_ROW_HEIGHT_FRACT * ui.ctx().viewport_rect().height())
        .min_col_width(available_width / 2.0)
        .striped(true)
        .show(ui, |ui| {
            if let Some(default_path) = default_path {
                ui.label(
                    RichText::new(default_path.to_string_lossy()).color(crate::FAINT_TEXT_COLOR),
                );
                ui.end_row();
            }

            let mut to_remove = None;

            for path in paths.iter() {
                let file_name = path
                    .file_name()
                    .map(|s| s.to_string_lossy())
                    .unwrap_or_else(|| "Invalid Path".into());
                ui.label(file_name);
                if ui.button("Remove").clicked() {
                    to_remove = Some(path.clone());
                    changed = true;
                }
                ui.end_row();
            }

            if let Some(to_remove) = to_remove {
                paths.retain(|p| p != &to_remove);
            }
        });

    changed
}

/// The font a subheading under a section heading is written in
fn subheading_font(ui: &egui::Ui) -> egui::FontId {
    let mut font = egui::TextStyle::Heading.resolve(ui.style());
    font.size *= SUBHEADING_SIZE_FRACTION;
    font
}

/// The Global EQ section: the EQs a user has saved for either side of the signal chain, which of
/// them is applied, and the editor for each of the applied ones.
///
/// Each side is a section of its own, under a subheading, as the two sides shape what is heard at
/// either end of the signal chain and so are kept, and chosen between, apart from each other.
///
/// The EQs are kept with the client's settings instead of on a processor, so they can be created,
/// edited and kept whether or not a processor is connected. Everything the editor changes is
/// therefore saved by the client, and only the EQ that is applied has to be sent to a processor.
///
/// Returns whether the EQ the processor should apply to that side of the signal chain changed.
fn global_eq_side_ui(
    ui: &mut egui::Ui,
    subheading: &str,
    note: &str,
    editor: &mut EqEditorState,
    id: u32,
    presets: &mut EqPresets,
) -> bool {
    // A selection that settings were saved with before its EQ was deleted applies nothing, and is
    // shown as such rather than as an EQ that is applied
    let selected = presets.selected;
    presets.select(selected);

    let mut changed = false;
    let available_width = ui.available_width();

    ui.label(RichText::new(subheading).font(subheading_font(ui)));
    ui.label(RichText::new(note).color(crate::FAINT_TEXT_COLOR));
    ui.add_space(20.0);

    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt((id, "global_eq_dropdown"))
            .width(available_width * 0.3)
            .selected_text(selected_eq_name(presets))
            .wrap_mode(egui::TextWrapMode::Truncate)
            .show_ui(ui, |ui| {
                changed |= ui
                    .selectable_value(&mut presets.selected, None, "No EQ")
                    .on_hover_text("Apply no EQ to the signal chain")
                    .changed();

                for (index, eq) in presets.eqs.iter().enumerate() {
                    changed |= ui
                        .selectable_value(&mut presets.selected, Some(index), &eq.name)
                        .changed();
                }
            });

        if ui
            .button("New")
            .on_hover_text("Create an EQ, named after one no other EQ is using")
            .clicked()
        {
            editor.name_input = Some(EqNameInput::new(None, presets.unused_name()));
        }

        let selected = presets.selected;

        // Renaming and deleting act on the EQ that is applied, and there is nothing to act on when
        // none is
        ui.add_enabled_ui(selected.is_some(), |ui| {
            if ui.button("Rename").clicked() {
                if let Some(eq) = presets.selected_eq() {
                    editor.name_input = Some(EqNameInput::new(selected, eq.name.clone()));
                }
            }

            if ui.button("Delete").clicked() {
                if let Some(index) = selected {
                    if presets.remove(index) {
                        // The EQs after the deleted one have moved up, so an open name field would
                        // be renaming a different EQ than the one it was opened for
                        editor.name_input = None;
                        changed = true;
                    }
                }
            }
        });
    });

    // The name field is taken out of the editor's state while it is edited, so the name that was
    // typed can be applied, or kept, once the field has been drawn
    if let Some(mut input) = editor.name_input.take() {
        ui.add_space(10.0);

        let mut confirmed = false;
        let mut cancelled = false;

        ui.horizontal(|ui| {
            ui.label(if input.rename.is_some() {
                "Rename EQ"
            } else {
                "New EQ"
            });

            let name_field = ui.add(
                egui::TextEdit::singleline(&mut input.name)
                    .desired_width(available_width * 0.3)
                    .hint_text("Name"),
            );

            // The field takes the keyboard as it opens, so the name that is offered can be typed
            // over right away
            if !input.focused {
                input.focused = true;
                name_field.request_focus();
            }

            let name_is_empty = input.name.trim().is_empty();
            let name_is_taken = eq_name_is_taken(presets, &input);

            confirmed = ui
                .add_enabled(!name_is_empty && !name_is_taken, egui::Button::new("OK"))
                .clicked()
                || (name_field.lost_focus()
                    && ui.input(|input| input.key_pressed(egui::Key::Enter)));
            cancelled = ui.button("Cancel").clicked()
                || ui.input(|input| input.key_pressed(egui::Key::Escape));

            if name_is_empty {
                ui.label(RichText::new("An EQ needs a name").color(Color32::RED));
            } else if name_is_taken {
                ui.label(
                    RichText::new("Another EQ is already using that name").color(Color32::RED),
                );
            }
        });

        ui.add_space(10.0);

        let name = input.name.trim().to_string();
        // Enter applies the name just like the button does, but not when the name was left empty,
        // or is one another EQ is using
        let name_is_taken = name.is_empty() || eq_name_is_taken(presets, &input);

        if confirmed && !name_is_taken {
            match input.rename {
                Some(index) => {
                    // Renaming changes nothing that is heard, so the processor needs nothing
                    presets.rename(index, name);
                }
                None => {
                    // A new EQ is applied as soon as it is created
                    presets.add(name);
                    changed = true;
                }
            }
        } else if !cancelled {
            // Keep the field open, holding what has been typed so far
            editor.name_input = Some(input);
        }
    }

    ui.add_space(20.0);

    if let Some(eq) = presets.selected_eq_mut() {
        changed |= graphic_eq_editor_ui(ui, eq, id);
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(
                RichText::new("No EQ is applied. Choose one, or create a new one.")
                    .color(crate::FAINT_TEXT_COLOR),
            );
        });
    }

    changed
}

/// The name the dropdown shows for the EQ that is applied
fn selected_eq_name(presets: &EqPresets) -> String {
    match presets.selected_eq() {
        Some(eq) => eq.name.clone(),
        None => "No EQ".to_string(),
    }
}

/// Whether the name being typed is one another EQ is already using, which would leave two entries
/// in the dropdown that cannot be told apart
fn eq_name_is_taken(presets: &EqPresets, input: &EqNameInput) -> bool {
    presets
        .eqs
        .iter()
        .enumerate()
        .any(|(index, eq)| eq.name == input.name.trim() && Some(index) != input.rename)
}

pub fn set_large_checkbox_style(ui: &mut egui::Ui) {
    ui.style_mut().spacing.icon_width = 35.0;
    ui.style_mut().spacing.icon_width_inner = 12.0;
    ui.style_mut().visuals.widgets.inactive.fg_stroke =
        egui::Stroke::new(2.0_f32, Color32::from_rgb(200, 200, 200));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Settings saved before the input and the output EQ were separate hold one set of EQs, under
    /// the key the output EQ had then, and are read as the EQs of the output side of the signal
    /// chain. They are saved again under the key each side is saved under now.
    #[test]
    fn eqs_saved_under_the_key_the_output_eq_had_are_read_as_the_output_eqs() {
        let saved = r#"{"global_eq":{"eqs":[{"name":"Saved"}],"selected":0}}"#;

        let settings: ClientSettings =
            serde_json::from_str(saved).expect("Failed to deserialize client settings");

        assert_eq!(
            settings
                .global_output_eq
                .selected_eq()
                .map(|eq| eq.name.as_str()),
            Some("Saved")
        );
        assert!(settings.global_input_eq.eqs.is_empty());

        let saved_again =
            serde_json::to_string(&settings).expect("Failed to serialize client settings");
        assert!(saved_again.contains(r#""global_output_eq""#));
        assert!(!saved_again.contains(r#""global_eq""#));
    }
}
