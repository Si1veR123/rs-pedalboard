use std::collections::HashMap;

use eframe::egui::{self, include_image};
use rs_pedalboard::pedals::{ParameterUILocation, Pedal, PedalDiscriminants, PedalParameter, PedalParameterValue, PedalTrait};
use crate::{midi::functions::ParameterMidiFunctionValues, socket::ParameterPath};

pub fn get_window_id(pedal: &Pedal) -> egui::Id {
    egui::Id::new("parameter_window").with(pedal.get_id())
}

pub fn get_window_open_id(pedal: &Pedal) -> egui::Id {
    get_window_id(pedal).with("open")
}

pub fn get_window_height_id(pedal: &Pedal) -> egui::Id {
    get_window_id(pedal).with("height")
}

pub fn get_parameter_settings_open_id(pedal: &Pedal, parameter_name: &str) -> egui::Id {
    get_window_id(pedal).with("parameter_settings_open").with(parameter_name)
}

pub fn get_parameter_settings_bg_id(pedal: &Pedal, parameter_name: &str) -> egui::Id {
    get_window_id(pedal).with("parameter_settings_bg").with(parameter_name)
}

pub fn get_minimum_parameter_id(pedal: &Pedal, parameter_name: &str) -> egui::Id {
    get_window_id(pedal).with("parameter_min").with(parameter_name)
}

pub fn get_maximum_parameter_id(pedal: &Pedal, parameter_name: &str) -> egui::Id {
    get_window_id(pedal).with("parameter_max").with(parameter_name)
}

pub fn get_selected_device_id(pedal: &Pedal, parameter_name: &str) -> egui::Id {
    get_window_id(pedal).with("selected_device").with(parameter_name)
}

pub enum ParameterWindowChange {
    ParameterChanged(String, PedalParameterValue),
    // Parameter path, new parameter MIDI function, device id
    AddMidiFunction(ParameterPath, ParameterMidiFunctionValues, u32),
    // Changed device on existing MIDI function. (parameter function, new device id, old device id)
    ChangeMidiFunctionDevice(ParameterPath, u32, u32),
    // Remove existing MIDI function (parameter path, device id)
    RemoveMidiFunction(ParameterPath, u32)
}

pub fn draw_parameter_window(ui: &mut egui::Ui, pedalboard_id: u32, pedal: &mut Pedal, devices: &HashMap<u32, String>) -> Option<ParameterWindowChange> {
    let id = get_window_id(pedal);
    let open_id = get_window_open_id(pedal);
    let height_id = get_window_height_id(pedal);
    let mut window_open = ui.ctx().data(|r| r.get_temp(open_id).unwrap_or(false));

    let mut to_change = None;

    egui::Window::new(PedalDiscriminants::from(&*pedal).display_name())
        .vscroll(true)
        .id(id)
        .open(&mut window_open)
        .collapsible(true)
        .min_height(300.0)
        .max_height(ui.ctx().data(|data| data.get_temp::<f32>(height_id).unwrap_or(600.0)))
        .show(ui.ctx(), |ui| {
            let mut parameters: Vec<_> = pedal.get_parameters().iter()
                .map(|(a, b)| (a.clone(), b.clone()))
                .collect();
            parameters.sort_by(|(a, _), (b, _)| a.cmp(b));

            let param_col_width = ui.max_rect().width() *0.9;
            egui::Grid::new(egui::Id::new("parameter_grid").with(pedal.get_id()))
                .num_columns(3)
                .min_row_height(40.0)
                .spacing(egui::vec2(0.0, 5.0))
                .show(ui, |ui| {
                    ui.style_mut().spacing.slider_width = param_col_width*0.8;
                    for (name, parameter) in parameters {
                        ui.label(&name);

                        if let Some(change) = pedal.parameter_editor_ui(ui, &name, &parameter, ParameterUILocation::ParameterWindow).inner {
                            to_change = Some(ParameterWindowChange::ParameterChanged(name.clone(), change));
                        }

                        // Parameter Function Button
                        let parameter_settings_button_id = get_parameter_settings_open_id(pedal, &name);
                        let mut is_selected = ui.ctx().data_mut(|d| *d.get_temp_mut_or(parameter_settings_button_id, false));
                        if ui.add_sized(
                            egui::Vec2::splat(30.0),
                            egui::ImageButton::new(include_image!("../files/settings_icon.png")).selected(is_selected)
                        ).clicked() {
                            is_selected = !is_selected;
                            ui.ctx().data_mut(|d| d.insert_temp(parameter_settings_button_id, is_selected));
                        };

                        ui.end_row();

                        if is_selected {
                            to_change = draw_midi_function_settings(ui, pedalboard_id, pedal, name, &parameter, devices);
                        }
                    }
                });
            
            ui.ctx().data_mut(|r| r.insert_temp(height_id, ui.min_size().y));
        });

    ui.ctx().data_mut(|r| r.insert_temp(open_id, window_open));

    to_change
}

// The state of the MIDI function settings is stored in persistent egui memory
pub fn draw_midi_function_settings(
    ui: &mut egui::Ui,
    pedalboard_id: u32,
    pedal: &mut Pedal,
    name: String,
    parameter: &PedalParameter,
    devices: &HashMap<u32, String>
) -> Option<ParameterWindowChange> {
    let last_frame_bg_rect = ui.ctx().data(|d| d.get_temp::<egui::Rect>(get_parameter_settings_bg_id(pedal, &name)).unwrap_or(egui::Rect::NOTHING));
    ui.painter().rect_filled(last_frame_bg_rect.expand(5.0), 3.0, egui::Color32::from_gray(40));

    let mut to_change = None;

    // This is a parameter that represents the minimum value set by MIDI
    let minimum_parameter_id = get_minimum_parameter_id(pedal, &name);
    let mut minimum_parameter = ui.ctx().data_mut(
        |d| d.get_persisted_mut_or_insert_with(minimum_parameter_id, || {
            // The default minimum parameter is a clone of the parameter, but set to its minimum value
            let mut minimum_parameter = parameter.clone();
            match &minimum_parameter.value {
                PedalParameterValue::Float(_) => {
                    let minimum_float = minimum_parameter.min.as_ref().unwrap().as_float().unwrap();
                    minimum_parameter.value = PedalParameterValue::Float(minimum_float);
                },
                PedalParameterValue::Int(_) => {
                    let minimum_int = minimum_parameter.min.as_ref().unwrap().as_int().unwrap();
                    minimum_parameter.value = PedalParameterValue::Int(minimum_int);
                },
                PedalParameterValue::Bool(_) => {
                    minimum_parameter.value = PedalParameterValue::Bool(false);
                },
                PedalParameterValue::String(_) => {
                    minimum_parameter.value = PedalParameterValue::String("".to_string());
                },
                _ => {}
            }
            minimum_parameter
        }).clone()
    );
    // This is a parameter that represents the maximum value set by MIDI
    let maximum_parameter_id = get_maximum_parameter_id(pedal, &name);
    let mut maximum_parameter = ui.ctx().data_mut(
        |d| d.get_persisted_mut_or_insert_with(maximum_parameter_id, || {
            // The default maximum parameter is a clone of the parameter, but set to its maximum value
            let mut maximum_parameter = parameter.clone();
            match maximum_parameter.value {
                PedalParameterValue::Float(_) => {
                    let maximum_float = maximum_parameter.max.as_ref().unwrap().as_float().unwrap();
                    maximum_parameter.value = PedalParameterValue::Float(maximum_float);
                },
                PedalParameterValue::Int(_) => {
                    let maximum_int = maximum_parameter.max.as_ref().unwrap().as_int().unwrap();
                    maximum_parameter.value = PedalParameterValue::Int(maximum_int);
                },
                PedalParameterValue::Bool(_) => {
                    maximum_parameter.value = PedalParameterValue::Bool(true);
                },
                PedalParameterValue::String(_) => {
                    maximum_parameter.value = PedalParameterValue::String("".to_string());
                },
                _ => {}
            }
            maximum_parameter
        }).clone()
    );

    // === Midi Device Selection ===
    let mut bg_rect = ui.label("MIDI Device").rect;

    let selected_device_data_id = get_selected_device_id(pedal, &name);
    let mut selected_device_id = ui.ctx().data_mut(|d| d.get_persisted_mut_or(selected_device_data_id, None).clone());

    let selected_device_name = if let Some(device_id) = selected_device_id {
        if let Some(name) = devices.get(&device_id).cloned() {
            Some(name)
        } else {
            // Device ID is no longer valid
            selected_device_id = None;
            ui.ctx().data_mut(|d| d.insert_persisted::<Option<u32>>(selected_device_data_id, None));
            None
        }
    } else {
        None
    };

    let old_selected_device_id = selected_device_id.clone();
    let combobox_rect = egui::ComboBox::from_id_salt(egui::Id::new("midi_device_select").with(pedal.get_id()).with(&name))
        .selected_text(selected_device_name.clone().unwrap_or_else(|| "None".to_string()))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut selected_device_id, None, "None");
            for device in devices {
                ui.selectable_value(&mut selected_device_id, Some(*device.0), device.1);
            }
        }).response.rect;
    bg_rect = bg_rect.union(combobox_rect);

    ui.end_row();

    if old_selected_device_id != selected_device_id {
        // Device has been changed. Can be either a new device, changing device or removing device
        ui.ctx().data_mut(|d| d.insert_persisted(selected_device_data_id, selected_device_id));

        match (old_selected_device_id, selected_device_id.clone()) {
            (Some(old_device), Some(new_device)) => {
                // Changing device
                to_change = Some(ParameterWindowChange::ChangeMidiFunctionDevice(
                    ParameterPath {
                        pedalboard_id,
                        pedal_id: pedal.get_id(),
                        parameter_name: name.clone()
                    },
                    new_device,
                    old_device
                ));
            },
            (Some(old_device), None) => {
                // Removing device
                to_change = Some(ParameterWindowChange::RemoveMidiFunction(
                    ParameterPath {
                        pedalboard_id,
                        pedal_id: pedal.get_id(),
                        parameter_name: name.clone()
                    },
                    old_device
                ));
            },
            (None, Some(new_device)) => {
                // New device
                to_change = Some(ParameterWindowChange::AddMidiFunction(
                    ParameterPath {
                        pedalboard_id,
                        pedal_id: pedal.get_id(),
                        parameter_name: name.clone()
                    },
                    ParameterMidiFunctionValues {
                        min_value: minimum_parameter.value.clone(),
                        max_value: maximum_parameter.value.clone()
                    },
                    new_device
                ));
            },
            _ => {}
        }
    }

    // === Min Value ===
    ui.label("Min Value");
    let min_changed = pedal.parameter_editor_ui(ui, &name, &minimum_parameter, ParameterUILocation::MidiMin).inner;
    ui.end_row();

    if let Some(mut change) = min_changed {
        // Minimum value has been changed
        // Ensure the minimum value does not exceed the maximum value
        match &mut change {
            PedalParameterValue::Float(min_value) => {
                *min_value = min_value.min(maximum_parameter.value.as_float().unwrap_or(*min_value));
            },
            PedalParameterValue::Int(ref mut min_value) => {
                *min_value = (*min_value).min(maximum_parameter.value.as_int().unwrap_or(*min_value));
            },
            _ => {}
        }

        if change != minimum_parameter.value {
            // Save the changed minimum value into memory
            minimum_parameter.value = change;
            ui.ctx().data_mut(|d| d.insert_persisted(minimum_parameter_id, minimum_parameter.clone()));

            // If a device is selected, update the MIDI function with the new minimum value
            if let Some(selected_device_id) = selected_device_id {
                to_change = Some(ParameterWindowChange::AddMidiFunction(
                    ParameterPath {
                        pedalboard_id,
                        pedal_id: pedal.get_id(),
                        parameter_name: name.clone()
                    },
                    ParameterMidiFunctionValues {
                        min_value: minimum_parameter.value.clone(),
                        max_value: maximum_parameter.value.clone()
                    },
                    selected_device_id
                ));
            }
        }
    }

    // === Max Value ===
    ui.label("Max Value");
    let egui::InnerResponse {
        inner: max_changed,
        response: max_parameter_response
    } = pedal.parameter_editor_ui(ui, &name, &maximum_parameter, ParameterUILocation::MidiMax);
    ui.end_row();

    bg_rect = bg_rect.union(max_parameter_response.rect);

    if let Some(mut change) = max_changed {
        // Maximum value has been changed
        // Ensure the maximum value does not go below the minimum value
        match &mut change {
            PedalParameterValue::Float(max_value) => {
                *max_value = max_value.max(minimum_parameter.value.as_float().unwrap_or(*max_value));
            },
            PedalParameterValue::Int(ref mut max_value) => {
                *max_value = (*max_value).max(minimum_parameter.value.as_int().unwrap_or(*max_value));
            },
            _ => {}
        }

        if change != maximum_parameter.value {
            // Save the changed maximum value into memory
            maximum_parameter.value = change;
            ui.ctx().data_mut(|d| d.insert_persisted(maximum_parameter_id, maximum_parameter.clone()));

            // If a device is selected, update the MIDI function with the new maximum value
            if let Some(selected_device_id) = selected_device_id {
                to_change = Some(ParameterWindowChange::AddMidiFunction(
                    ParameterPath {
                        pedalboard_id,
                        pedal_id: pedal.get_id(),
                        parameter_name: name.clone()
                    },
                    ParameterMidiFunctionValues {
                        min_value: minimum_parameter.value.clone(),
                        max_value: maximum_parameter.value.clone()
                    },
                    selected_device_id
                ));
            }
        }
    }

    // bg_rect covers the top left to the bottom right of the parameter settings
    // save it in context memory for next frame
    ui.data_mut(|d| d.insert_temp(get_parameter_settings_bg_id(pedal, &name), bg_rect));

    to_change
}
