pub mod functions;
use rs_pedalboard::unique_time_id;
use strum::IntoEnumIterator;

use std::{collections::{HashMap, HashSet}, sync::{atomic::AtomicU32, Arc, Mutex}};
use midir::{MidiInput, MidiInputConnection, MidiInputPort};
use serde::{Serialize, Deserialize, Serializer, Deserializer, ser::SerializeStruct};
use eframe::egui::{self, Id, Rangef, RichText};
use egui_extras::{Size, StripBuilder};
use crossbeam::channel::Sender;

use crate::{midi::{functions::{GlobalMidiFunction, ParameterMidiFunctionValues}}, socket::{ClientSocketThreadHandle, Command, ParameterPath}, SAVE_DIR};

pub const MIDI_SETTINGS_SAVE_NAME: &'static str = "midi_settings.json";

pub struct MidiState {
    settings: Arc<Mutex<MidiSettings>>,
    // Name, Id, Connection
    input_connections: Vec<(String, String, MidiInputConnection<String>)>,
    available_input_ports: Vec<(String, MidiInputPort)>, // (name, port)
    ui_thread_sender: Sender<Command>,
    socket_handle: Option<ClientSocketThreadHandle>,
    pub active_pedalboard_id: Arc<AtomicU32>,
    egui_ctx: egui::Context
}

impl MidiState {
    pub fn new(
        settings: MidiSettings,
        egui_ctx: egui::Context,
        ui_thread_sender:
        Sender<Command>,
        socket_handle: Option<ClientSocketThreadHandle>,
        active_pedalboard_id: u32
    ) -> Self {
        let available_named_input_ports = Self::resolve_port_names(Self::create_midi_input());

        Self {
            settings: Arc::new(Mutex::new(settings)),
            available_input_ports: available_named_input_ports,
            input_connections: Vec::new(),
            socket_handle,
            egui_ctx,
            active_pedalboard_id: Arc::new(AtomicU32::new(active_pedalboard_id)),
            ui_thread_sender
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
        ctx.data_mut(|d| { d.insert_temp(egui::Id::new("midi_device_cache_invalid"), true); });
    }

    pub fn connect_to_auto_connect_ports(&mut self) {
        let settings_lock = self.settings.lock().expect("MidiState: Mutex poisoned.");
        let auto_connect_ports: Vec<String> = settings_lock.port_settings.iter()
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
        self.settings.lock().map_err(|_e| std::io::Error::new(std::io::ErrorKind::Other, "MIDI settings mutex poisoned"))?.save()
    }

    fn create_midi_input() -> MidiInput {
        MidiInput::new("Pedalboard MIDI Input").expect("Failed to create MIDI input")
    }

    fn parse_cc_message(message: &[u8]) -> Option<(u8, u8, u8)> {
        if message.len() < 3 || message[0] & 0xF0 != 0xB0 {
            return None; // Not a Control Change message
        }
        Some(((message[0] & 0x0F) + 1, message[1], message[2]))
    }

    fn device_settings_mut<'a>(settings: &'a mut MidiSettings, port_id: &str, cc: u8, channel: u8, ctx: &egui::Context) -> Option<&'a mut MidiDevice> {
        if let Some(settings) = settings.port_settings.get_mut(port_id) {
            Some(settings.devices.entry((cc, channel)).or_insert_with(|| {
                Self::invalidate_device_name_cache(ctx);

                MidiDevice {
                    id: unique_time_id(),
                    name: "New Device".to_string(),
                    device_type: MidiDeviceType::AbsoluteEncoder { min_value: 0, max_value: 127 },
                    current_value: 0.5,
                    global_functions: Vec::new(),
                    parameter_functions: HashMap::new(),
                    use_global: true
                }
            }))
        } else {
            None
        }
    }

    fn handle_midi_message(
        settings: &Arc<Mutex<MidiSettings>>,
        port_id: &str,
        message: &[u8],
        ui_thread_sender: &Sender<Command>,
        socket_handle: Option<&ClientSocketThreadHandle>,
        egui_ctx: &egui::Context,
        active_pedalboard_id: u32
    ) {
        let (channel, cc, value) = match Self::parse_cc_message(message) {
            Some((channel, cc, value)) => (channel, cc, value),
            None => return
        };

        log::debug!("Received MIDI CC message on port ID '{}': channel {}, cc {}, value {}", port_id, channel, cc, value);

        let mut settings_lock = settings.lock().expect("MidiState: Mutex poisoned.");

        if let Some(device) = Self::device_settings_mut(&mut settings_lock, port_id, cc, channel, egui_ctx) {
            let old_value = device.current_value;
            device.update_with_midi_value(value);
            egui_ctx.request_repaint();
            if device.current_value != old_value {
                // Activate any MIDI functions for this device
                if device.use_global {
                    for function in &device.global_functions {
                        let command = function.command_from_function(device.current_value);
                        if let Err(e) = ui_thread_sender.send(command.clone()) {
                            log::error!("Failed to send global MIDI command to UI thread: {}", e);
                        }

                        if let Some(handle) = &socket_handle {
                            handle.send_command(command);
                        }
                    }
                } else {
                    for (path, function_values) in &device.parameter_functions {
                        if path.pedalboard_id != active_pedalboard_id {
                            continue;
                        }

                        let command = Command::ParameterUpdate(path.clone(), function_values.parameter_from_value(device.current_value));
                        if let Err(e) = ui_thread_sender.send(command.clone()) {
                            log::error!("Failed to send parameter MIDI command to UI thread: {}", e);
                        }

                        if let Some(handle) = &socket_handle {
                            handle.send_command(command);
                        }
                    }
                }
            }
        }
    }

    pub fn connect_to_port(&mut self, id: &str) {
        if let Some((port_name, port)) = self.available_input_ports.iter().find(|(_name, p)| p.id() == id) {
            if !self.input_connections.iter().any(|(_name, conn_id, _c) | conn_id == id) {
                let midi_input = Self::create_midi_input();
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
                            active_pedalboard_id_clone.load(std::sync::atomic::Ordering::Relaxed)
                        );
                    },
                    id.to_string()
                ) {
                    Ok(connection) => {
                        self.input_connections.push((
                            port_name.clone(),
                            id.to_string(),
                            connection
                        ));
                        log::info!("Connected to MIDI port: {}", id);
                        self.available_input_ports.retain(|(_name, p)| p.id() != id);
                        self.settings.lock().expect("MidiState: Mutex poisoned.").port_settings.entry(id.to_string()).or_default();
                    }
                    Err(e) => {
                        log::error!("Failed to connect to MIDI port {}: {}", id, e);
                        return;
                    }
                }
            }
        } else {
            log::error!("MIDI port {} not found", id);
        }
    }

    pub fn disconnect_from_all_ports(&mut self) {
        self.input_connections.clear();
        self.refresh_available_ports();
    }

    pub fn disconnect_from_port(&mut self, id: &str) {
        self.input_connections.retain(|(_name, conn_id, _)| conn_id != id);
        self.refresh_available_ports();
    }

    fn resolve_port_names(midi_input: MidiInput) -> Vec<(String, MidiInputPort)> {
        let ports = midi_input.ports();
        ports.into_iter()
            .map(|p| (midi_input.port_name(&p).unwrap_or_else(|_e| p.id().to_string()), p))
            .collect()
    }

    pub fn refresh_available_ports(&mut self) {
        self.available_input_ports = Self::resolve_port_names(Self::create_midi_input());
        self.available_input_ports.retain(
            // Remove any ports that we are already connected to
            |(_name, p)| !self.input_connections.iter().any(|(_name, conn_id, _)| conn_id == &p.id())
        );
    }

    pub fn remove_old_parameter_functions(&self, existing_pedalboards: &HashSet<u32>) {
        let mut settings_lock = self.settings.lock().expect("MidiState: Mutex poisoned.");

        for (_port_name, port_settings) in settings_lock.port_settings.iter_mut() {
            for (_cc_channel, device) in port_settings.devices.iter_mut() {
                device.parameter_functions.retain(|f, _| existing_pedalboards.contains(&f.pedalboard_id));
            }
        }
    }

    pub fn add_midi_parameter_function_to_device(
        &self,
        parameter_path: ParameterPath,
        midi_function_values: ParameterMidiFunctionValues,
        device_id: u32
    ) {
        let mut settings_lock = self.settings.lock().expect("MidiState: Mutex poisoned.");

        for (_port_id, port_settings) in settings_lock.port_settings.iter_mut() {
            for (_, device) in port_settings.devices.iter_mut() {
                if device.id == device_id {
                    device.parameter_functions.insert(parameter_path.clone(), midi_function_values);
                    return;
                }
            }
        }

        log::warn!("MIDI device ID '{}' not found when adding MIDI function", device_id);
    }

    pub fn remove_midi_parameter_function_from_device(
        &self,
        parameter: &ParameterPath,
        device_id: u32
    ) -> Option<ParameterMidiFunctionValues> {
        let mut settings_lock = self.settings.lock().expect("MidiState: Mutex poisoned.");

        for (_port_id, port_settings) in settings_lock.port_settings.iter_mut() {
            for ((_cc, _channel), device) in port_settings.devices.iter_mut() {
                if device.id == device_id {
                    return device.parameter_functions.remove(parameter);
                }
            }
        }

        log::warn!("MIDI device ID '{}' not found when removing MIDI function", device_id);

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
            .min_col_width(ui.available_width()/2.0)
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Available MIDI Ports:");
                ui.button("Refresh").on_hover_text("Refresh available MIDI ports").clicked().then(|| self.refresh_available_ports());
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
    pub port_settings: HashMap<String, MidiPortSettings>
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
        let stringified = serde_json::to_string(self).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let dir_path = homedir::my_home().map_err(
            |e| std::io::Error::new(std::io::ErrorKind::Other, e)
        )?.unwrap().join(SAVE_DIR);

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
                log::error!("Could not determine home directory, using default MIDI settings");
                return Default::default();
            }
            Err(e) => {
                log::error!("Failed to get home directory: {e}, using default MIDI settings");
                return Default::default();
            }
        };

        if !file_path.exists() {
            log::info!("MIDI Settings save file not found at {:?}, using default", file_path);
            return Default::default();
        }

        let stringified = match std::fs::read_to_string(&file_path) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to read MIDI settings file {:?}: {e}, using default", file_path);
                return Default::default();
            }
        };

        match serde_json::from_str(&stringified) {
            Ok(state) => state,
            Err(e) => {
                log::error!("Failed to deserialize MIDI settings from {:?}: {e}, using default", file_path);
                Default::default()
            }
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
    pub parameter_functions: HashMap<ParameterPath, ParameterMidiFunctionValues>,
    pub use_global: bool,
}

impl MidiDevice {
    pub fn update_with_midi_value(&mut self, midi_value: u8) {
        match &self.device_type {
            MidiDeviceType::RelativeEncoder { sensitivity, increment_value, decrement_value } => {
                if midi_value == *increment_value {
                    self.current_value += *sensitivity;
                } else if midi_value == *decrement_value {
                    self.current_value -= *sensitivity;
                }
                self.current_value = self.current_value.clamp(0.0, 1.0);
            }
            MidiDeviceType::AbsoluteEncoder { min_value, max_value } => {
                let range = *max_value as f32 - *min_value as f32;
                self.current_value = (midi_value as f32 - *min_value as f32) / range;
            }
            MidiDeviceType::Footswitch {
                on_value,
                momentary_to_latching
            } => {
                self.current_value = if *momentary_to_latching {
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
                }
            }
        }
    }

    pub fn display_value_string(&self) -> String {
        match &self.device_type {
            MidiDeviceType::RelativeEncoder { .. } | MidiDeviceType::AbsoluteEncoder { .. } => {
                format!("{:.2}", self.current_value)
            },
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MidiDeviceType {
    RelativeEncoder {
        sensitivity: f32,
        increment_value: u8,
        decrement_value: u8,
    },
    AbsoluteEncoder {
        min_value: u8,
        max_value: u8
    },
    Footswitch {
        on_value: u8,
        momentary_to_latching: bool
    }
}

impl MidiDeviceType {
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
            MidiDeviceType::RelativeEncoder { sensitivity, increment_value, decrement_value } => {
                ui.label("Sensitivity:");
                ui.add(egui::Slider::new(sensitivity, 0.01..=1.0));
                ui.end_row();
                ui.label("Increment Value:");
                ui.add(egui::Slider::new(increment_value, 0..=127));
                ui.end_row();
                ui.label("Decrement Value:");
                ui.add(egui::Slider::new(decrement_value, 0..=127));
                ui.end_row();
            },
            MidiDeviceType::AbsoluteEncoder { min_value, max_value } => {
                ui.label("Min Value:");
                ui.add(egui::Slider::new(min_value, 0..=127));
                ui.end_row();
                ui.label("Max Value:");
                ui.add(egui::Slider::new(max_value, 0..=127));
                ui.end_row();
            },
            MidiDeviceType::Footswitch { on_value, momentary_to_latching } => {
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
