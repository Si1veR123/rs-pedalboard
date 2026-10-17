use std::{collections::HashMap, hash::Hash};

use eframe::egui::{self, include_image, Button, Color32, Image, Vec2};
use serde::{Deserialize, Serialize};

use super::{gain_knob, SLOT_ASPECT_RATIO, get_band_image_from_value, get_band_type_next_value, get_band_type_from_value};
use crate::{
    dsp::eq::{Equalizer, GraphicEqualizerBuilder, BandType}, pedalboard::ParameterPath, pedals::{PedalParameter, PedalParameterValue, PedalTrait}, unique_time_id,
};

const BAND_FREQS: [f32; 5] = [100.0, 400.0, 1000.0, 3000.0, 6500.0];
const MAX_GAIN_DB: f32 = 15.0;
const BAND_Q: f32 = 0.5;

#[derive(Serialize, Deserialize, Clone)]
pub struct SimpleEq {
    parameters: HashMap<String, PedalParameter>,
    #[serde(skip)]
    eq: Option<Equalizer>,
    #[serde(skip)]
    sample_rate: f32,
    id: u32,
}

impl Hash for SimpleEq {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl SimpleEq {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        let init_gain = 0.0;
        for i in 0..7 {
            parameters.insert(
                format!("Gain {}", i + 1),
                PedalParameter {
                    value: PedalParameterValue::Float(init_gain),
                    min: Some(PedalParameterValue::Float(-MAX_GAIN_DB)),
                    max: Some(PedalParameterValue::Float(MAX_GAIN_DB)),
                    step: Some(PedalParameterValue::Float(0.1)),
                },
            );
        }

        parameters.insert(
            "Low Band Mode".to_string(),
            PedalParameter {
                value: PedalParameterValue::Int(0),
                min: Some(PedalParameterValue::Int(0)),
                max: Some(PedalParameterValue::Int(2)),
                step: None,
            },
        );

        parameters.insert(
            "High Band Mode".to_string(),
            PedalParameter {
                value: PedalParameterValue::Int(0),
                min: Some(PedalParameterValue::Int(0)),
                max: Some(PedalParameterValue::Int(2)),
                step: None,
            },
        );

        parameters.insert(
            "Active".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None,
            },
        );

        Self {
            parameters,
            eq: None,
            sample_rate: 48000.0,
            id: unique_time_id(),
        }
    }

    fn parameter_float(&self, name: &str) -> f32 {
        self.parameters
            .get(name)
            .and_then(|parameter| parameter.value.as_float())
            .unwrap_or(0.0)
    }

    fn parameter_int(&self, name: &str) -> i16 {
        self.parameters
            .get(name)
            .and_then(|parameter| parameter.value.as_int())
            .unwrap_or(0)
    }

    fn build_eq(&self) -> Equalizer {
        let mut builder = GraphicEqualizerBuilder::<5>::new(self.sample_rate)
            .with_bands(BAND_FREQS)
            .with_gains([
                self.parameter_float("Gain 1"),
                self.parameter_float("Gain 2"),
                self.parameter_float("Gain 3"),
                self.parameter_float("Gain 4"),
                self.parameter_float("Gain 5"),
            ])
            .with_bandwidths([BAND_Q; 5]);
        
        let low_band_mode = self.parameter_int("Low Band Mode");
        let high_band_mode = self.parameter_int("High Band Mode");

        builder = builder
            .with_lower_band_type(get_band_type_from_value(low_band_mode).unwrap_or(BandType::Peaking))
            .with_upper_band_type(get_band_type_from_value(high_band_mode).unwrap_or(BandType::Peaking));

        builder.build()
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned.eq = None;
        cloned
    }
}

impl PedalTrait for SimpleEq {
    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.eq.is_none() {
            self.eq = Some(self.build_eq());
        }

        for sample in buffer.iter_mut() {
            *sample = self.eq.as_mut().unwrap().process(*sample);
        }
    }

    fn parameter_value_display_string(
        &self,
        name: &str,
        value: &PedalParameterValue,
    ) -> Option<String> {
        match name {
            "Low Gain" | "Mid Gain" | "High Gain" => {
                Some(format!("{:+.1} dB", value.as_float().unwrap()))
            }
            "High Band Mode" | "Low Band Mode" => {
                let band_type = get_band_type_from_value(value.as_int().unwrap()).unwrap_or(BandType::Peaking);
                Some(format!("{:?}", band_type))
            }
            _ => None,
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        if let Some(parameter) = self.parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                parameter.value = value;
                
                if name.starts_with("Gain ") || name == "Low Band Mode" || name == "High Band Mode" {
                    self.eq = Some(self.build_eq());
                }
            }
        }
    }

    fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
        self.sample_rate = sample_rate as f32;
        self.eq = Some(self.build_eq());
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _message_buffer: &[String],
    ) -> Option<(String, PedalParameterValue)> {
        let pedal_rect = ui.available_rect_before_wrap();
        
        ui.new_child(egui::UiBuilder::new().max_rect(pedal_rect))
            .add(Image::new(include_image!("../images/simple_eq.png")));

        let mut changed = None;
        let pedal_size = pedal_rect.size();
        let low_band_mode = self.parameter_int("Low Band Mode");
        let high_band_mode = self.parameter_int("High Band Mode");

        let top_section_height = pedal_size.y * 0.075;
        ui.allocate_ui_with_layout(Vec2::new(pedal_rect.width(), top_section_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::from_black_alpha(50);
            ui.style_mut().visuals.widgets.hovered.weak_bg_fill = Color32::from_black_alpha(80);
            ui.columns_const(|[left, _, right]| {
                for (column, mode, name) in [
                    (left, low_band_mode, "Low Band Mode"),
                    (right, high_band_mode, "High Band Mode"),
                ] {
                    if column
                        .centered_and_justified(|ui| {
                            ui.add(
                                Button::image(
                                    Image::new(get_band_image_from_value(mode))
                                        .max_width(pedal_size.x * 0.15),
                                )
                                .corner_radius(3.0),
                            )
                            .on_hover_text(name)
                            .clicked()
                        })
                        .inner
                    {
                        changed = Some((
                            name.to_string(),
                            PedalParameterValue::Int(get_band_type_next_value(mode)),
                        ));
                    }
                }
            });
        });

        // The pedal is allocated slightly taller than the `eq.png` artwork, which is drawn
        // top-aligned and leaves a blank strip at the bottom. Bound the knob row to the
        // visible artwork so the knobs are centered within the pedal itself.
        let background_size = Image::new(include_image!("../images/eq.png"))
            .load_and_calc_size(ui, pedal_size)
            .unwrap_or(pedal_size);
        let artwork_bottom = pedal_rect.top() + background_size.y - pedal_size.y * 0.05; // subtract 5% for frequency band text at bottom
        let knob_area = egui::Vec2::new(
            ui.available_width(),
            (artwork_bottom - ui.cursor().top()).max(0.0),
        );

        // The knob slot image (`slot.png`) is 39x298 px, so its width is ~13% of its
        // height. Use that to size the knobs so the slots use the full vertical space.
        const SLOT_WIDTH_TO_HEIGHT_RATIO: f32 = 39.0 / 298.0;

        ui.allocate_ui_with_layout(
            knob_area,
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                // Make the slots as wide as the available height allows so they fill it,
                // but never wider than a single one of the five columns (a reasonable width).
                let knob_width = (knob_area.y * SLOT_WIDTH_TO_HEIGHT_RATIO)
                    .min(knob_area.x / 5.0)
                    .max(1.0);
                let knob_height = knob_width / SLOT_WIDTH_TO_HEIGHT_RATIO;

                // Center the row of knobs vertically within the available space
                ui.add_space(((knob_area.y - knob_height) / 2.0).max(0.0));

                ui.columns(5, |columns| {
                    for (column, (name, enabled)) in columns.iter_mut().zip([
                        ("Gain 1", low_band_mode != 2),
                        ("Gain 2", true),
                        ("Gain 3", true),
                        ("Gain 4", true),
                        ("Gain 5", high_band_mode != 2),
                    ]) {
                        column.vertical_centered(|ui| {
                            if enabled {
                                // The slot is drawn at the size of the range a band's gain is set
                                // over, in the proportions of the image it is drawn from, and the
                                // knob as wide as it, so the knob travels over the whole of it
                                if let Some(value) = gain_knob(
                                    ui,
                                    self.parameters.get(name).unwrap(),
                                    Vec2::new(knob_width, knob_width * SLOT_ASPECT_RATIO),
                                    knob_width,
                                    1.0,
                                    Some(&ParameterPath {
                                        pedalboard_id: None,
                                        pedal_id: self.id,
                                        parameter_name: name.to_string(),
                                    }),
                                ) {
                                    changed =
                                        Some((name.to_string(), PedalParameterValue::Float(value)));
                                }
                            }
                        });
                    }
                });
            },
        );

        changed
    }
}
