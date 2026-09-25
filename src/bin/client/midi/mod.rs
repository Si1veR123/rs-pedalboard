pub mod functions;
use rs_pedalboard::pedals::parameters::{ParameterUpdate, PedalParameterRange};
use rs_pedalboard::unique_time_id;
use rs_pedalboard::{pedalboard::ParameterPath, processor_settings::FloatSettingUpdate};
use strum::IntoEnumIterator;
use strum_macros::EnumDiscriminants;

use crossbeam::channel::Sender;
use eframe::egui::{self, Id, Rangef, RichText};
use egui_extras::{Size, StripBuilder};
use midir::{MidiInput, MidiInputConnection, MidiInputPort};
use serde::{ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};
use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::AtomicU32, Arc, Mutex},
};

use crate::popup_message::popup;
use crate::{
    midi::functions::GlobalMidiFunction,
    socket::{ClientSocketThreadHandle, Command},
    SAVE_DIR,
};

pub const MIDI_SETTINGS_SAVE_NAME: &'static str = "midi_settings.json";

pub struct MidiState {
    settings: Arc<Mutex<MidiSettings>>,
    // Name, Id, Connection
    input_connections: Vec<(String, String, MidiInputConnection<String>)>,
    available_input_ports: Vec<(String, MidiInputPort)>, // (name, port)
    ui_thread_sender: Sender<Command>,
    socket_handle: Option<ClientSocketThreadHandle>,
    pub active_pedalboard_id: Arc<AtomicU32>,
    egui_ctx: egui::Context,
}

impl MidiState {
    pub fn new(
        settings: MidiSettings,
        egui_ctx: egui::Context,
        ui_thread_sender: Sender<Command>,
        socket_handle: Option<ClientSocketThreadHandle>,
        active_pedalboard_id: u32,
    ) -> Self {
        let mut available_named_input_ports = vec![];
        if let Ok(input) = Self::create_midi_input(Some(egui_ctx.clone())) {
            available_named_input_ports = Self::resolve_port_names(input);
        }

        Self {
            settings: Arc::new(Mutex::new(settings)),
            available_input_ports: available_named_input_ports,
            input_connections: Vec::new(),
            socket_handle,
            egui_ctx,
            active_pedalboard_id: Arc::new(AtomicU32::new(active_pedalboard_id)),
            ui_thread_sender,
        }
    }

    pub fn get_all_parameter_devices(&self) -> HashMap<u32, String> {
        let settings_lock = self.settings.lock().expect("MidiState: Mutex poisoned.");
        let mut device_names = HashMap::new();
        for (_port_id, port_settings) in settings_lock.port_settings.iter() {
            for ((_cc, _channel), device) in port_settings.devices.iter() {
                if !device.use_global {
                    device_names.insert(device.id, device.name.clone());
                }
            }
        }
        device_names
    }

    pub fn invalidate_device_name_cache(ctx: &egui::Context) {
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new("midi_device_cache_invalid"), true);
        });
    }

    pub fn connect_to_auto_connect_ports(&mut self) {
        let settings_lock = self.settings.lock().expect("MidiState: Mutex poisoned.");
        let auto_connect_ports: Vec<String> = settings_lock
            .port_settings
            .iter()
            .filter_map(|(port_id, port_settings)| {
                if port_settings.auto_connect {
                    Some(port_id.clone())
                } else {
                    None
                }
            })
            .collect();
        drop(settings_lock);

        for port in auto_connect_ports {
            self.connect_to_port(&port);
        }
    }

    pub fn set_socket_handle(&mut self, handle: Option<ClientSocketThreadHandle>) {
        self.socket_handle = handle;
    }

    pub fn save_settings(&self) -> Result<(), std::io::Error> {
        self.settings
            .lock()
            .map_err(|_e| {
                std::io::Error::new(std::io::ErrorKind::Other, "MIDI settings mutex poisoned")
            })?
            .save()
    }

    fn create_midi_input(ctx: Option<egui::Context>) -> Result<MidiInput, midir::InitError> {
        match MidiInput::new("Pedalboard MIDI Input") {
            Ok(input) => Ok(input),
            Err(e) => {
                tracing::error!("Failed to create MIDI input: {}", e);
                if let Some(egui_ctx) = ctx {
                    popup!(
                        egui_ctx,
                        "Failed to create MIDI input",
                        Some("midi-input-error"),
                        crate::popup_message::PopupMessageType::Error
                    );
                }
                Err(e)
            }
        }
    }

    fn parse_cc_message(message: &[u8]) -> Option<(u8, u8, u8)> {
        if message.len() < 3 || message[0] & 0xF0 != 0xB0 {
            return None; // Not a Control Change message
        }
        Some(((message[0] & 0x0F) + 1, message[1], message[2]))
    }

    fn handle_midi_message(
        settings: &Arc<Mutex<MidiSettings>>,
        port_id: &str,
        message: &[u8],
        ui_thread_sender: &Sender<Command>,
        socket_handle: Option<&ClientSocketThreadHandle>,
        egui_ctx: &egui::Context,
        active_pedalboard_id: u32,
    ) {
        let (channel, cc, value) = match Self::parse_cc_message(message) {
            Some((channel, cc, value)) => (channel, cc, value),
            None => return,
        };

        tracing::debug!(
            "Received MIDI CC message on port ID '{}': channel {}, cc {}, value {}",
            port_id,
            channel,
            cc,
            value
        );

        let mut settings_lock = settings.lock().expect("MidiState: Mutex poisoned.");

        // Mutable phase
        let change = {
            let Some(device) = settings_lock.device_settings_mut(port_id, cc, channel, egui_ctx)
            else {
                return;
            };

            let change = device.change_from_midi_value(value);

            if change == MidiChange::None || change == MidiChange::RelativeValueChange(0.0) {
                tracing::debug!("No change to MIDI device.");
                return;
            }

            change
        };

        egui_ctx.request_repaint();

        // Read phase
        let (function_mode, global_functions, parameter_functions) = {
            let Some(device) = settings_lock
                .port_settings
                .get(port_id)
                .and_then(|port_settings| port_settings.devices.get(&(cc, channel)))
            else {
                return;
            };

            (
                device.function_mode(active_pedalboard_id),
                device.global_functions.clone(),
                device.parameter_functions.clone(),
            )
        };

        match function_mode {
            DeviceFunctionMode::Global => {
                // Activate any global MIDI functions for this device
                for function in &global_functions {
                    let command = function.command_from_function(&change);
                    if let Some(command) = command {
                        match ui_thread_sender.send(command.clone()) {
                            Ok(()) => {
                                if let Some(handle) = &socket_handle {
                                    handle.send_command(command);

                                    if function.should_show_popup() {
                                        // Do the popup here instead of in state, so that it is only shown when triggered by MIDI
                                        // A popup shouldnt be shown when the user clicks a button in the UI
                                        popup!(egui_ctx.clone(), format!("MIDI: {}", function));
                                    }
                                }
                            },
                            Err(e) => {
                                tracing::error!(
                                    "Failed to send global MIDI command to UI thread: {}",
                                    e
                                );
                            }
                        }
                    }
                }
            }
            DeviceFunctionMode::Parameter => {
                // Activate the parameter functions of this device
                for (path, function_values) in &parameter_functions {
                    if path.pedalboard_id != Some(active_pedalboard_id) {
                        continue;
                    }

                    let parameter_update = change.to_parameter_update(function_values);
                    if let Some(parameter_update) = parameter_update {
                        let command = Command::ParameterUpdate(path.clone(), parameter_update);
                        match ui_thread_sender.send(command.clone()) {
                            Ok(()) => {
                                if let Some(handle) = &socket_handle {
                                    handle.send_command(command);
                                }
                            },
                            Err(e) => {
                                tracing::error!(
                                    "Failed to send parameter MIDI command to UI thread: {}",
                                    e
                                );
                            }
                        }
                    }
                }
            }
            DeviceFunctionMode::Sensible => {
                // The device has no functions of its own, fall back to a sensible parameter
                let device_index = settings_lock.get_parameter_device_index(
                    port_id,
                    cc,
                    channel,
                    active_pedalboard_id,
                );
                if let Some(device_index) = device_index {
                    if let Some(float_update) = change.to_float_setting_update() {
                        let command =
                            Command::SensibleMidiParameterUpdate(device_index, float_update);
                        if let Err(e) = ui_thread_sender.send(command.clone()) {
                            tracing::error!(
                                "Failed to send sensible MIDI parameter update command to UI thread: {}",
                                e
                            );
                        }
                    }
                }
            }
        }
    }

    pub fn connect_to_port(&mut self, id: &str) {
        if let Some((port_name, port)) = self
            .available_input_ports
            .iter()
            .find(|(_name, p)| p.id() == id)
        {
            if !self
                .input_connections
                .iter()
                .any(|(_name, conn_id, _c)| conn_id == id)
            {
                let midi_input = match Self::create_midi_input(Some(self.egui_ctx.clone())) {
                    Ok(input) => input,
                    Err(_) => {
                        return;
                    }
                };

                let settings_clone = self.settings.clone();
                let ui_thread_sender_clone = self.ui_thread_sender.clone();
                let socket_thread_handle_clone = self.socket_handle.clone();
                let active_pedalboard_id_clone = self.active_pedalboard_id.clone();
                let egui_ctx_clone = self.egui_ctx.clone();
                match midi_input.connect(
                    port,
                    "Pedalboard MIDI Input Port",
                    move |_time, message, data| {
                        Self::handle_midi_message(
                            &settings_clone,
                            data.as_str(),
                            message,
                            &ui_thread_sender_clone,
                            socket_thread_handle_clone.as_ref(),
                            &egui_ctx_clone,
                            active_pedalboard_id_clone.load(std::sync::atomic::Ordering::Relaxed),
                        );
                    },
                    id.to_string(),
                ) {
                    Ok(connection) => {
                        self.input_connections.push((
                            port_name.clone(),
                            id.to_string(),
                            connection,
                        ));
                        tracing::info!("Connected to MIDI port: {}", id);
                        self.available_input_ports.retain(|(_name, p)| p.id() != id);
                        self.settings
                            .lock()
                            .expect("MidiState: Mutex poisoned.")
                            .port_settings
                            .entry(id.to_string())
                            .or_default();
                    }
                    Err(e) => {
                        tracing::error!("Failed to connect to MIDI port {}: {}", id, e);
                        return;
                    }
                }
            }
        } else {
            tracing::error!("MIDI port {} not found", id);
            popup!(
                self.egui_ctx.clone(),
                format!("MIDI port {} not found", id),
                Some("midi-port-not-found"),
                crate::popup_message::PopupMessageType::Error
            );
        }
    }

    pub fn disconnect_from_all_ports(&mut self) {
        self.input_connections.clear();
        self.refresh_available_ports();
    }

    pub fn disconnect_from_port(&mut self, id: &str) {
        self.input_connections
            .retain(|(_name, conn_id, _)| conn_id != id);
        self.refresh_available_ports();
    }

    fn resolve_port_names(midi_input: MidiInput) -> Vec<(String, MidiInputPort)> {
        let ports = midi_input.ports();
        ports
            .into_iter()
            .map(|p| {
                (
                    midi_input
                        .port_name(&p)
                        .unwrap_or_else(|_e| p.id().to_string()),
                    p,
                )
            })
            .collect()
    }

    pub fn refresh_available_ports(&mut self) {
        let midi_input = match Self::create_midi_input(Some(self.egui_ctx.clone())) {
            Ok(input) => input,
            Err(_) => {
                return;
            }
        };
        self.available_input_ports = Self::resolve_port_names(midi_input);
        self.available_input_ports.retain(
            // Remove any ports that we are already connected to
            |(_name, p)| {
                !self
                    .input_connections
                    .iter()
                    .any(|(_name, conn_id, _)| conn_id == &p.id())
            },
        );
    }

    pub fn remove_old_parameter_functions(&self, existing_pedalboards: &HashSet<u32>) {
        let mut settings_lock = self.settings.lock().expect("MidiState: Mutex poisoned.");

        for (_port_name, port_settings) in settings_lock.port_settings.iter_mut() {
            for (_cc_channel, device) in port_settings.devices.iter_mut() {
                device.parameter_functions.retain(|f, _| {
                    f.pedalboard_id
                        .is_some_and(|pedalboard_id| existing_pedalboards.contains(&pedalboard_id))
                });
            }
        }
    }

    pub fn add_midi_parameter_function_to_device(
        &self,
        parameter_path: ParameterPath,
        midi_function_values: PedalParameterRange,
        device_id: u32,
    ) {
        let mut settings_lock = self.settings.lock().expect("MidiState: Mutex poisoned.");

        for (_port_id, port_settings) in settings_lock.port_settings.iter_mut() {
            for (_, device) in port_settings.devices.iter_mut() {
                if device.id == device_id {
                    device
                        .parameter_functions
                        .insert(parameter_path.clone(), midi_function_values);
                    return;
                }
            }
        }

        tracing::warn!(
            "MIDI device ID '{}' not found when adding MIDI function",
            device_id
        );
    }

    pub fn remove_midi_parameter_function_from_device(
        &self,
        parameter: &ParameterPath,
        device_id: u32,
    ) -> Option<PedalParameterRange> {
        let mut settings_lock = self.settings.lock().expect("MidiState: Mutex poisoned.");

        for (_port_id, port_settings) in settings_lock.port_settings.iter_mut() {
            for ((_cc, _channel), device) in port_settings.devices.iter_mut() {
                if device.id == device_id {
                    return device.parameter_functions.remove(parameter);
                }
            }
        }

        tracing::warn!(
            "MIDI device ID '{}' not found when removing MIDI function",
            device_id
        );

        None
    }

    /// This UI contains a list of ports that we can connect to, and a list of connected ports.
    /// Connected ports have a list of devices from MidiSettings, that can be removed, edited, etc.
    pub fn midi_port_device_settings_ui(&mut self, ui: &mut egui::Ui) {
        let row_height = 60.0;

        ui.add_space(10.0);
        egui::Grid::new("midi_ports_grid")
            .striped(true)
            .min_row_height(row_height)
            .min_col_width(ui.available_width() / 2.0)
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Available MIDI Ports:");
                ui.button("Refresh")
                    .on_hover_text("Refresh available MIDI ports")
                    .clicked()
                    .then(|| self.refresh_available_ports());
                ui.end_row();

                if self.available_input_ports.is_empty() {
                    ui.label("No available MIDI input ports found");
                    ui.end_row();
                } else {
                    let mut connect = None;

                    for (name, port) in &self.available_input_ports {
                        ui.label(name);
                        if ui.button("Connect").clicked() {
                            connect = Some(port.id());
                        }
                        ui.end_row();
                    }

                    if let Some(port_id) = connect {
                        self.connect_to_port(&port_id);
                    }
                }
            });

        ui.add_space(40.0);

        ui.label("Connected MIDI Ports:");

        let mut settings_lock = self.settings.lock().expect("MidiState: Mutex poisoned.");

        let row_count = {
            let mut row_count = self.input_connections.len();
            for (_port_name, port_id, _connection) in &self.input_connections {
                if let Some(settings) = settings_lock.port_settings.get(port_id) {
                    row_count += settings.devices.len();
                }
            }
            row_count
        };

        let mut disconnect: Option<String> = None;
        let row_height = 60.0;
        StripBuilder::new(ui)
            .sizes(Size::Absolute { initial: row_height, range: Rangef::new(0.0, row_height) }, row_count)
            .vertical(|mut strip| {
                for (port_name, port_id, _connection) in &self.input_connections {
                    // Port summary
                    strip.cell(|ui| {
                        ui.painter().rect_filled(ui.available_rect_before_wrap(), 5.0, crate::LIGHT_BACKGROUND_COLOR);
                        let width = ui.available_width();
                        StripBuilder::new(ui)
                            .size(Size::Absolute { initial: width*0.5, range: Rangef::new(0.0, width*0.5) }) // Port name
                            .size(Size::Absolute { initial: width*0.25, range: Rangef::new(0.0, width*0.25) }) // Disconnect
                            .size(Size::Absolute { initial: width*0.25, range: Rangef::new(0.0, width*0.25) }) // Auto-connect
                            .horizontal(|mut strip| {
                                strip.cell(|ui| { ui.horizontal_centered(|ui| ui.label(port_name.as_str())); });
                                strip.cell(|ui| {
                                    if ui.horizontal_centered(|ui| ui.button("Disconnect")).inner.clicked() {
                                        disconnect = Some(port_id.clone());
                                    }
                                });
                                strip.cell(|ui| {
                                    let port_settings = settings_lock.port_settings.get_mut(port_id).expect("Any connected port should have an entry in port settings.");
                                    ui.horizontal_centered(|ui| ui.toggle_value(&mut port_settings.auto_connect, "Auto-Connect"));
                                });
                            });
                    });

                    // Device rows for this port
                    if let Some(device_settings) = settings_lock.port_settings.get_mut(port_id) {
                        let mut forget: Option<(u8, u8)> = None;

                        for (i, ((cc, channel), device)) in device_settings.devices.iter_mut().enumerate() {
                            // Device summary row
                            strip.cell(|ui| {
                                // Use the rect saved in the last frame to paint the background
                                let mut rect = ui.ctx().memory(|m| m.data.get_temp::<egui::Rect>(Id::new("device_rect").with(i)).unwrap_or(ui.available_rect_before_wrap()));
                                rect.set_width(ui.available_width());
                                if i % 2 == 0 {
                                    ui.painter().rect_filled(rect, 5.0, crate::LIGHT_BACKGROUND_COLOR.gamma_multiply(0.6));
                                }
                                StripBuilder::new(ui)
                                    .size(Size::Absolute { initial: row_height, range: Rangef::new(0.0, row_height) }) // Device name etc.
                                    .size(Size::Absolute { initial: 40.0, range: Rangef::new(0.0, 40.0) }) // Device settings collapsing header
                                    .vertical(|mut strip| {
                                        strip.strip(|builder| {
                                            builder
                                                .size(Size::Absolute { initial: rect.width()*0.75, range: Rangef::new(0.0, rect.width()*0.75) })
                                                .size(Size::Absolute { initial: rect.width()*0.25, range: Rangef::new(0.0, rect.width()*0.25) })

                                                .horizontal(|mut strip| {
                                                    strip.cell(|ui| {
                                                        ui.horizontal_centered(|ui| ui.label(
                                                            RichText::new(
                                                                format!("{} - CC {cc} Ch {channel}", &device.name)
                                                            ).color(crate::FAINT_TEXT_COLOR)
                                                        ));
                                                    });
                                                    strip.cell(|ui| {
                                                        ui.horizontal_centered(|ui| {
                                                            if ui.button("Forget").clicked() {
                                                                forget = Some((*cc, *channel));
                                                            }
                                                        });
                                                    });
                                                });
                                        });

                                        // Full-width row for collapsible details/settings
                                        strip.cell(|ui| {
                                            ui.push_id((port_id.as_str(), cc, channel), |ui| {
                                                ui.vertical_centered(|ui| {
                                                    egui::CollapsingHeader::new("Device Settings")
                                                    .id_salt(egui::Id::new("midi_device_settings").with(i))
                                                    .show(ui, |ui| {
                                                        ui.add_space(5.0);

                                                        egui::Grid::new(egui::Id::new("midi_device_settings_grid").with(i))
                                                            .num_columns(2)
                                                            .min_col_width(ui.available_width()/2.0)
                                                            .min_row_height(40.0)
                                                            .show(ui, |ui| {
                                                                ui.label("Current Value:");
                                                                ui.label(device.display_value_string());
                                                                ui.end_row();

                                                                ui.label("Rename:");
                                                                if ui.text_edit_singleline(&mut device.name).changed() {
                                                                    Self::invalidate_device_name_cache(&self.egui_ctx);
                                                                }
                                                                ui.end_row();

                                                                ui.label("Device Type:");
                                                                egui::ComboBox::from_id_salt(egui::Id::new("midi_device_type").with(i))
                                                                    .selected_text(device.device_type.get_name())
                                                                    .show_ui(ui, |ui| {
                                                                        if ui
                                                                            .selectable_label(
                                                                                matches!(device.device_type, MidiDeviceType::RelativeEncoder { .. }),
                                                                                "Relative Encoder",
                                                                            )
                                                                            .clicked()
                                                                        {
                                                                            device.device_type = MidiDeviceType::RelativeEncoder {
                                                                                sensitivity: 0.1,
                                                                                increment_value: 0,
                                                                                decrement_value: 127,
                                                                            };
                                                                        }
                                                                        if ui
                                                                            .selectable_label(
                                                                                matches!(device.device_type, MidiDeviceType::AbsoluteEncoder { .. }),
                                                                                "Absolute Encoder",
                                                                            )
                                                                            .clicked()
                                                                        {
                                                                            device.device_type = MidiDeviceType::AbsoluteEncoder {
                                                                                min_value: 0,
                                                                                max_value: 127,
                                                                            };
                                                                        }
                                                                        if ui
                                                                            .selectable_label(
                                                                                matches!(device.device_type, MidiDeviceType::Footswitch { .. }),
                                                                                "Footswitch",
                                                                            )
                                                                            .clicked()
                                                                        {
                                                                            device.device_type = MidiDeviceType::Footswitch {
                                                                                on_value: 127,
                                                                                momentary_to_latching: false
                                                                            };
                                                                        }
                                                                    });
                                                                ui.end_row();

                                                                device.device_type.settings_ui(ui);

                                                                ui.label("Use Global Functions:");

                                                                ui.scope(|ui| {
                                                                    crate::settings::set_large_checkbox_style(ui);
                                                                    if ui.checkbox(&mut device.use_global, "")
                                                                        .on_hover_text("If enabled, the global functions will be used. If disabled, the parameter functions will be used.")
                                                                        .changed() {
                                                                            Self::invalidate_device_name_cache(&self.egui_ctx);
                                                                        }
                                                                });
                                                                ui.end_row();

                                                                if device.use_global {
                                                                    ui.label("");
                                                                    egui::ComboBox::from_id_salt(&device.name)
                                                                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                                                                        .selected_text(format!("{} functions", device.global_functions.len()))
                                                                        .show_ui(ui, |ui| {
                                                                            let selected_color = ui.visuals().selection.bg_fill;

                                                                            for global_function in GlobalMidiFunction::iter() {
                                                                                let is_active = device.global_functions.contains(&global_function);
                                                                                let bg_color = if is_active { selected_color } else { ui.visuals().widgets.inactive.bg_fill };
                                                                                if ui.selectable_label(
                                                                                    is_active,
                                                                                    RichText::new(format!("{}", global_function)).background_color(bg_color)
                                                                                ).clicked() {
                                                                                    if is_active {
                                                                                        device.global_functions.retain(|f| f != &global_function);
                                                                                    } else {
                                                                                        device.global_functions.push(global_function.clone());
                                                                                    }
                                                                                };
                                                                            }
                                                                        });
                                                                    ui.end_row();
                                                                }
                                                            }
                                                        );
                                                        ui.add_space(5.0);
                                                    });
                                                });
                                            });
                                        });
                                    });
                                let min_rect = ui.min_rect();
                                ui.ctx().memory_mut(|m| {
                                    m.data.insert_temp(Id::new("device_rect").with(i), min_rect);
                                });
                            });
                        }

                        if let Some((cc, channel)) = forget {
                            Self::invalidate_device_name_cache(&self.egui_ctx);
                            device_settings.devices.remove(&(cc, channel));
                        }
                    }
                }
            });
        drop(settings_lock);

        if let Some(port_id) = disconnect {
            self.disconnect_from_port(&port_id);
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MidiSettings {
    // Port ID, Settings
    pub port_settings: HashMap<String, MidiPortSettings>,
}

impl MidiSettings {
    fn device_settings_mut<'a>(
        &'a mut self,
        port_id: &str,
        cc: u8,
        channel: u8,
        ctx: &egui::Context,
    ) -> Option<&'a mut MidiDevice> {
        if let Some(settings) = self.port_settings.get_mut(port_id) {
            Some(settings.devices.entry((cc, channel)).or_insert_with(|| {
                MidiState::invalidate_device_name_cache(ctx);

                MidiDevice {
                    id: unique_time_id(),
                    name: "New Device".to_string(),
                    device_type: MidiDeviceType::AbsoluteEncoder {
                        min_value: 0,
                        max_value: 127,
                    },
                    current_value: 0.5,
                    global_functions: Vec::new(),
                    parameter_functions: HashMap::new(),
                    use_global: true,
                }
            }))
        } else {
            None
        }
    }

    /// The index of the device at `(port_id, cc, channel)` among the devices which have no
    /// functions of their own and therefore fall back to a sensible parameter mapping.
    ///
    /// Devices are numbered per [`MidiDeviceKind`] and ordered by `(port_id, cc, channel)` so
    /// that a device keeps its index across runs (the underlying maps are unordered) and when
    /// its configured settings change. The first device of a kind has index 0.
    fn get_parameter_device_index(
        &self,
        port_id: &str,
        cc: u8,
        channel: u8,
        active_pedalboard_id: u32,
    ) -> Option<usize> {
        let target_kind = {
            let target = self
                .port_settings
                .get(port_id)?
                .devices
                .get(&(cc, channel))?;

            if target.function_mode(active_pedalboard_id) != DeviceFunctionMode::Sensible {
                return None;
            }

            target.device_type.kind()
        };

        let mut device_keys: Vec<(&str, (u8, u8))> = self
            .port_settings
            .iter()
            .flat_map(|(port_id, port_settings)| {
                port_settings
                    .devices
                    .keys()
                    .map(move |cc_channel| (port_id.as_str(), *cc_channel))
            })
            .collect();
        device_keys.sort_unstable();

        let mut index = 0;
        for (device_port_id, cc_channel) in device_keys {
            let device = &self.port_settings[device_port_id].devices[&cc_channel];

            if device_port_id == port_id && cc_channel == (cc, channel) {
                // `index` counts the sensibly mapped devices of this kind before the target,
                // which is the target's own index
                return Some(index);
            }

            if device.device_type.kind() == target_kind
                && device.function_mode(active_pedalboard_id) == DeviceFunctionMode::Sensible
            {
                index += 1;
            }
        }

        None
    }
}

#[derive(Debug, Clone)]
pub struct MidiPortSettings {
    // (cc, channel)
    pub devices: HashMap<(u8, u8), MidiDevice>,
    pub auto_connect: bool,
}

impl Default for MidiPortSettings {
    fn default() -> Self {
        MidiPortSettings {
            devices: HashMap::new(),
            auto_connect: true,
        }
    }
}

impl Serialize for MidiPortSettings {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert the HashMap<(cc, ch), MidiDevice> into HashMap<String, &MidiDevice>
        let converted: HashMap<String, &MidiDevice> = self
            .devices
            .iter()
            .map(|(port, inner)| {
                let key = format!("{}:{}", port.0, port.1);
                (key, inner)
            })
            .collect();

        let mut struct_serializer = serializer.serialize_struct("Port", 2)?;
        struct_serializer.serialize_field("devices", &converted)?;
        struct_serializer.serialize_field("auto_connect", &self.auto_connect)?;

        struct_serializer.end()
    }
}

impl<'de> Deserialize<'de> for MidiPortSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Port {
            devices: HashMap<String, MidiDevice>,
            auto_connect: bool,
        }

        // first deserialize into HashMap<String, HashMap<String, MidiDevice>>
        let raw: Port = Deserialize::deserialize(deserializer)?;

        let mut actual_map = HashMap::new();
        for (key, dev) in raw.devices {
            let mut parts = key.split(':');
            let cc = parts
                .next()
                .ok_or_else(|| serde::de::Error::custom("missing cc"))?
                .parse::<u8>()
                .map_err(serde::de::Error::custom)?;
            let ch = parts
                .next()
                .ok_or_else(|| serde::de::Error::custom("missing channel"))?
                .parse::<u8>()
                .map_err(serde::de::Error::custom)?;
            actual_map.insert((cc, ch), dev);
        }

        Ok(MidiPortSettings {
            devices: actual_map,
            auto_connect: raw.auto_connect,
        })
    }
}

impl MidiSettings {
    pub fn save(&self) -> Result<(), std::io::Error> {
        let stringified = serde_json::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let dir_path = homedir::my_home()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
            .unwrap()
            .join(SAVE_DIR);

        if !dir_path.exists() {
            std::fs::create_dir_all(&dir_path)?;
        }
        let file_path = dir_path.join(MIDI_SETTINGS_SAVE_NAME);

        std::fs::write(file_path, stringified)
    }

    pub fn load_or_default() -> Self {
        let file_path = match homedir::my_home() {
            Ok(Some(home)) => home.join(SAVE_DIR).join(MIDI_SETTINGS_SAVE_NAME),
            Ok(None) => {
                tracing::error!("Could not determine home directory, using default MIDI settings");
                return Default::default();
            }
            Err(e) => {
                tracing::error!("Failed to get home directory: {e}, using default MIDI settings");
                return Default::default();
            }
        };

        if !file_path.exists() {
            tracing::info!(
                "MIDI Settings save file not found at {:?}, using default",
                file_path
            );
            return Default::default();
        }

        let stringified = match std::fs::read_to_string(&file_path) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(
                    "Failed to read MIDI settings file {:?}: {e}, using default",
                    file_path
                );
                return Default::default();
            }
        };

        match serde_json::from_str(&stringified) {
            Ok(state) => state,
            Err(e) => {
                tracing::error!(
                    "Failed to deserialize MIDI settings from {:?}: {e}, using default",
                    file_path
                );
                Default::default()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MidiChange {
    RelativeValueChange(f32),
    AbsoluteValueChange(f32),
    Toggled,
    None,
}

impl MidiChange {
    pub fn to_float_setting_update(&self) -> Option<FloatSettingUpdate> {
        match self {
            MidiChange::RelativeValueChange(v) => Some(FloatSettingUpdate::Relative(*v)),
            MidiChange::AbsoluteValueChange(v) => Some(FloatSettingUpdate::Absolute(*v)),
            MidiChange::Toggled => Some(FloatSettingUpdate::FlipFlop),
            MidiChange::None => None,
        }
    }

    pub fn to_parameter_update(&self, range: &PedalParameterRange) -> Option<ParameterUpdate> {
        match self {
            MidiChange::RelativeValueChange(v) => {
                Some(ParameterUpdate::Relative(*v, Some(range.clone())))
            }
            MidiChange::AbsoluteValueChange(v) => {
                let parameter_value = range.parameter_from_interp(*v);
                Some(ParameterUpdate::Absolute(parameter_value))
            }
            MidiChange::Toggled => Some(ParameterUpdate::FlipFlop(Some(range.clone()))),
            MidiChange::None => None,
        }
    }
}

use serde_with::{serde_as, Seq};
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiDevice {
    pub id: u32,
    pub name: String,
    pub device_type: MidiDeviceType,
    pub current_value: f32,
    pub global_functions: Vec<GlobalMidiFunction>,
    #[serde_as(as = "Seq<(_, _)>")]
    pub parameter_functions: HashMap<ParameterPath, PedalParameterRange>,
    pub use_global: bool,
}

impl MidiDevice {
    pub fn change_from_midi_value(&mut self, midi_value: u8) -> MidiChange {
        match &self.device_type {
            MidiDeviceType::RelativeEncoder {
                sensitivity,
                increment_value,
                decrement_value,
            } => {
                if midi_value == *increment_value {
                    self.current_value += *sensitivity;
                    self.current_value = self.current_value.clamp(0.0, 1.0);
                    MidiChange::RelativeValueChange(*sensitivity)
                } else if midi_value == *decrement_value {
                    self.current_value -= *sensitivity;
                    self.current_value = self.current_value.clamp(0.0, 1.0);
                    MidiChange::RelativeValueChange(-*sensitivity)
                } else {
                    tracing::debug!("Received MIDI value {midi_value} for relative encoder, but it is not the increment ({increment_value}) or decrement ({decrement_value}) value. Ignoring.");
                    MidiChange::None
                }
            }
            MidiDeviceType::AbsoluteEncoder {
                min_value,
                max_value,
            } => {
                let range = *max_value as f32 - *min_value as f32;
                let new_value = (midi_value as f32 - *min_value as f32) / range;
                self.current_value = new_value.clamp(0.0, 1.0);
                MidiChange::AbsoluteValueChange(self.current_value)
            }
            MidiDeviceType::Footswitch {
                on_value,
                momentary_to_latching,
            } => {
                let new_state = if *momentary_to_latching {
                    if midi_value == *on_value {
                        if self.current_value == 0.0 {
                            1.0
                        } else {
                            0.0
                        }
                    } else {
                        self.current_value
                    }
                } else {
                    if midi_value == *on_value {
                        1.0
                    } else {
                        0.0
                    }
                };
                let toggled = new_state != self.current_value;
                self.current_value = new_state;
                if !toggled {
                    MidiChange::None
                } else if *momentary_to_latching {
                    MidiChange::Toggled
                } else {
                    // A momentary footswitch follows its position instead of toggling,
                    // otherwise press and release would each toggle the parameter.
                    MidiChange::AbsoluteValueChange(new_state)
                }
            }
        }
    }

    pub fn function_mode(&self, active_pedalboard_id: u32) -> DeviceFunctionMode {
        if self.use_global {
            DeviceFunctionMode::Global
        } else if self
            .parameter_functions
            .iter()
            // Filter to parameter functions which are on the active pedalboard
            .filter(|(path, _range)| path.pedalboard_id == Some(active_pedalboard_id))
            .count()
            == 0
        {
            DeviceFunctionMode::Sensible
        } else {
            DeviceFunctionMode::Parameter
        }
    }

    pub fn display_value_string(&self) -> String {
        match &self.device_type {
            MidiDeviceType::RelativeEncoder { .. } | MidiDeviceType::AbsoluteEncoder { .. } => {
                format!("{:.2}", self.current_value)
            }
            MidiDeviceType::Footswitch { .. } => {
                if self.current_value == 1.0 {
                    "On".into()
                } else {
                    "Off".into()
                }
            }
        }
    }
}

/// Which mappings a [`MidiDevice`]'s messages are routed through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceFunctionMode {
    /// The device's global functions.
    Global,
    /// The device's per-parameter functions.
    Parameter,
    /// The device has no functions of its own, so a sensible parameter is chosen instead.
    Sensible,
}
#[derive(Debug, Clone, Serialize, Deserialize, EnumDiscriminants)]
#[strum_discriminants(name(MidiDeviceKind))]
pub enum MidiDeviceType {
    RelativeEncoder {
        sensitivity: f32,
        increment_value: u8,
        decrement_value: u8,
    },
    AbsoluteEncoder {
        min_value: u8,
        max_value: u8,
    },
    Footswitch {
        on_value: u8,
        momentary_to_latching: bool,
    },
}

impl MidiDeviceType {
    /// The kind of this device type, without its per-device settings.
    pub fn kind(&self) -> MidiDeviceKind {
        match self {
            MidiDeviceType::RelativeEncoder { .. } => MidiDeviceKind::RelativeEncoder,
            MidiDeviceType::AbsoluteEncoder { .. } => MidiDeviceKind::AbsoluteEncoder,
            MidiDeviceType::Footswitch { .. } => MidiDeviceKind::Footswitch,
        }
    }

    pub fn get_name(&self) -> &'static str {
        match self {
            MidiDeviceType::RelativeEncoder { .. } => "Relative Encoder",
            MidiDeviceType::AbsoluteEncoder { .. } => "Absolute Encoder",
            MidiDeviceType::Footswitch { .. } => "Footswitch",
        }
    }

    /// UI is built for an egui Grid with 2 columns.
    pub fn settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.style_mut().spacing.slider_width = ui.available_width() * 0.8;
        match self {
            MidiDeviceType::RelativeEncoder {
                sensitivity,
                increment_value,
                decrement_value,
            } => {
                ui.label("Sensitivity:");
                ui.add(egui::Slider::new(sensitivity, 0.01..=1.0));
                ui.end_row();
                ui.label("Increment Value:");
                ui.add(egui::Slider::new(increment_value, 0..=127));
                ui.end_row();
                ui.label("Decrement Value:");
                ui.add(egui::Slider::new(decrement_value, 0..=127));
                ui.end_row();
            }
            MidiDeviceType::AbsoluteEncoder {
                min_value,
                max_value,
            } => {
                ui.label("Min Value:");
                ui.add(egui::Slider::new(min_value, 0..=127));
                ui.end_row();
                ui.label("Max Value:");
                ui.add(egui::Slider::new(max_value, 0..=127));
                ui.end_row();
            }
            MidiDeviceType::Footswitch {
                on_value,
                momentary_to_latching,
            } => {
                ui.label("On Value:");
                ui.add(egui::Slider::new(on_value, 0..=127));
                ui.end_row();
                ui.label("Convert Momentary To Latching:");
                ui.scope(|ui| {
                    crate::settings::set_large_checkbox_style(ui);
                    ui.checkbox(momentary_to_latching, "")
                });
                ui.end_row();
            }
        }
    }
}
