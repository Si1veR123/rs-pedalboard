use eframe::egui;
use rs_pedalboard::{dsp_algorithms::oscillator::{self, Oscillator}, pedals::{Pedal, PedalDiscriminants, PedalParameter, PedalParameterValue, PedalTrait}};

pub fn get_window_open_id(pedal: &Pedal) -> egui::Id {
    egui::Id::new("parameter_window").with(pedal.get_id())
}

pub enum ParameterWindowChange {
    ParameterChanged(String, PedalParameterValue)
}

pub fn draw_parameter_window(ui: &mut egui::Ui, pedal: &Pedal) -> Option<ParameterWindowChange> {
    let id = get_window_open_id(pedal);
    let mut window_open = ui.ctx().data(|r| r.get_temp(id).unwrap_or(false));

    let mut to_change = None;

    egui::Window::new(PedalDiscriminants::from(pedal).display_name())
        .vscroll(true)
        .id(id)
        .open(&mut window_open)
        .collapsible(true)
        .show(ui.ctx(), |ui| {
            let mut parameters: Vec<_> = pedal.get_parameters().iter().collect();
            parameters.sort_by(|(a, _), (b, _)| a.cmp(b));

            for (name, parameter) in parameters {
                match parameter.value {
                    PedalParameterValue::Float(mut f) => {
                        let init_value = f;
                        let min = parameter.min.clone().unwrap().as_float().unwrap_or(0.0);
                        let max = parameter.max.clone().unwrap().as_float().unwrap_or(1.0);
                        ui.add(egui::Slider::new(&mut f, min..=max).text(name));

                        if f != init_value {
                            to_change = Some(ParameterWindowChange::ParameterChanged(name.clone(), PedalParameterValue::Float(f)));
                        }
                    }
                    PedalParameterValue::Bool(mut b) => {
                        let init_value = b;
                        ui.checkbox(&mut b, name);
                        if b != init_value {
                            to_change = Some(ParameterWindowChange::ParameterChanged(name.clone(), PedalParameterValue::Bool(b)));
                        }
                    }
                    PedalParameterValue::Int(mut i) => {
                        let init_value = i;
                        let min = parameter.min.clone().unwrap().as_int().unwrap_or(0);
                        let max = parameter.max.clone().unwrap().as_int().unwrap_or(100);
                        ui.add(egui::Slider::new(&mut i, min..=max).text(name));

                        if i != init_value {
                            to_change = Some(ParameterWindowChange::ParameterChanged(name.clone(), PedalParameterValue::Int(i)));
                        }
                    }
                    PedalParameterValue::Oscillator(_) => {
                        if let Some(oscillator) = oscillator_selection_window(ui, name, &parameter, pedal.get_id(), false) {
                            to_change = Some(ParameterWindowChange::ParameterChanged(name.clone(), PedalParameterValue::Oscillator(oscillator)));
                        }
                    }

                    _ => {
                        
                    }
                }
            }
        });

    ui.ctx().data_mut(|r| r.insert_temp(id, window_open));

    to_change
}

pub fn oscillator_selection_window(
    ui: &mut egui::Ui,
    parameter_name: &str,
    parameter: &PedalParameter,
    id: u32,
    oscillator_type_only: bool
) -> Option<Oscillator> {
    let selected_oscillator = parameter.value.as_oscillator().unwrap();
    let mut new_oscillator = None;

    egui::CollapsingHeader::new(parameter_name)
        .id_salt(egui::Id::new("oscillator_selection").with(id).with(parameter_name))
        .show(ui, |ui| {
            ui.columns(4, |columns| {
                let [sine_ui, square_ui, sawtooth_ui, triangle_ui] = &mut columns[..] else { unreachable!() };
    
                if matches!(selected_oscillator, Oscillator::Sine(_)) {
                    sine_ui.add(egui::Button::new("Sine").selected(true));
                } else if sine_ui.add(egui::Button::new("Sine")).clicked() {
                    new_oscillator = Some(Oscillator::Sine(oscillator::Sine::new(
                        // Sample rate on oscillator parameters on client do not matter, it is set correctly on the server
                        48000.0,
                        selected_oscillator.get_frequency(),
                        selected_oscillator.get_phase_offset(),
                        0.0,
                    )));
                }
    
                if matches!(selected_oscillator, Oscillator::Square(_)) {
                    square_ui.add(egui::Button::new("Square").selected(true));
                } else if square_ui.add(egui::Button::new("Square")).clicked() {
                    new_oscillator = Some(Oscillator::Square(oscillator::Square::new(
                        48000.0,
                        selected_oscillator.get_frequency(),
                        selected_oscillator.get_phase_offset(),
                    )));
                }
    
                if matches!(selected_oscillator, Oscillator::Sawtooth(_)) {
                    sawtooth_ui.add(egui::Button::new("Sawtooth").selected(true));
                } else if sawtooth_ui.add(egui::Button::new("Sawtooth")).clicked() {
                    new_oscillator = Some(Oscillator::Sawtooth(oscillator::Sawtooth::new(
                        48000.0,
                        selected_oscillator.get_frequency(),
                        selected_oscillator.get_phase_offset(),
                    )));
                }
    
                if matches!(selected_oscillator, Oscillator::Triangle(_)) {
                    triangle_ui.add(egui::Button::new("Triangle").selected(true));
                } else if triangle_ui.add(egui::Button::new("Triangle")).clicked() {
                    new_oscillator = Some(Oscillator::Triangle(oscillator::Triangle::new(
                        48000.0,
                        selected_oscillator.get_frequency(),
                        selected_oscillator.get_phase_offset(),
                    )));
                }
            });
    
            if !oscillator_type_only {
                // Frequency
                let mut frequency_value = selected_oscillator.get_frequency();

                let frequency_range = {
                    let min_freq = parameter.min.as_ref().and_then(|p| p.as_float()).unwrap_or(0.0);
                    let max_freq = parameter.max.as_ref().and_then(|p| p.as_float()).unwrap_or(20.0);
                    min_freq..=max_freq
                };
                ui.add(egui::Slider::new(&mut frequency_value, frequency_range).logarithmic(true).text("Frequency"));
    
                if frequency_value != selected_oscillator.get_frequency() {
                    let mut cloned = selected_oscillator.clone();
                    cloned.set_frequency(frequency_value);
                    new_oscillator = Some(cloned);
                }
    
                // Phase
                let mut phase_offset_value = selected_oscillator.get_phase_offset();
                ui.add(egui::Slider::new(&mut phase_offset_value, -0.5..=0.5).text("Phase Offset"));
    
                if phase_offset_value != selected_oscillator.get_phase_offset() {
                    let mut cloned = selected_oscillator.clone();
                    cloned.set_phase_offset(phase_offset_value);
                    new_oscillator = Some(cloned);
                }
    
                // Sine-specific squareness
                if let Oscillator::Sine(sine) = selected_oscillator {
                    let mut squareness_value = sine.get_squareness();
                    ui.add(egui::Slider::new(&mut squareness_value, 0.0..=1.0).text("Squareness"));
                    if squareness_value != sine.get_squareness() {
                        let new_sine = oscillator::Sine::new(
                            48000.0,
                            sine.frequency.0,
                            sine.phase_offset.0,
                            squareness_value,
                        );
                        new_oscillator = Some(Oscillator::Sine(new_sine));
                    }
                }
            }
        });

    new_oscillator
}