use eframe::egui;
use eframe::egui::Vec2;

use super::{PedalParameter, PedalParameterValue};


pub fn pedal_knob(ui: &mut egui::Ui, name: &str, parameter: &PedalParameter) -> Option<PedalParameterValue> {
    match parameter.value {
        PedalParameterValue::Float(_) => {
            let mut value = parameter.value.clone().as_float().unwrap();
            let init_value = value.clone();
            let min = parameter.min.clone().unwrap().as_float().unwrap();
            let max = parameter.max.clone().unwrap().as_float().unwrap();
            let step = parameter.step.clone().and_then(|s| Some(s.as_float().unwrap()));

            ui.label(name);

            let mut slider = egui::Slider::new(&mut value, min..=max);
            if let Some(step) = step {
                slider = slider.step_by(step as f64);
            }

            ui.add_sized(Vec2::new(75.0, 25.0), slider);

            if value != init_value {
                Some(PedalParameterValue::Float(value))
            } else {
                None
            }
        },
        PedalParameterValue::Int(_) => {
            let mut value = parameter.value.clone().as_int().unwrap();
            let init_value = value.clone();
            let min = parameter.min.clone().unwrap().as_int().unwrap();
            let max = parameter.max.clone().unwrap().as_int().unwrap();

            ui.label(name);

            let slider = egui::Slider::new(&mut value, min..=max).step_by(1.0);
            ui.add_sized(Vec2::new(75.0, 25.0), slider);

            if value != init_value {
                Some(PedalParameterValue::Int(value))
            } else {
                None
            }
        },
        _ => {
            ui.label("Invalid parameter type.");
            return None;
        }
    }

    
}
