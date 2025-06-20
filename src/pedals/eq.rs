use std::collections::HashMap;
use std::hash::Hash;

use eframe::egui::{self, include_image, Color32, Image, ImageButton, RichText, UiBuilder, Vec2};
use egui_plot::{HLine, Line, Plot, PlotPoint, VLine};
use serde::{Deserialize, Serialize};

use super::{PedalParameter, PedalParameterValue, PedalTrait};

use crate::{dsp_algorithms::eq::{self, Equalizer}, pedals::ui::pedal_knob, unique_time_id};

#[derive(Clone)]
pub struct GraphicEq7 {
    parameters: HashMap<String, PedalParameter>,
    eq: eq::Equalizer,
    id: usize,
    response_plot: Vec<PlotPoint>
}

impl Hash for GraphicEq7 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
        self.id.hash(state);
    }
}

impl Serialize for GraphicEq7 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_map(self.parameters.iter())
    }
}

impl<'a> Deserialize<'a> for GraphicEq7 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let parameters: HashMap<String, PedalParameter> = HashMap::deserialize(deserializer)?;

        let high_shelf_enabled = parameters.get("high_shelf")
            .and_then(|p| p.value.as_float())
            .map_or(true, |v| v > 0.0);
        let low_shelf_enabled = parameters.get("low_shelf")
            .and_then(|p| p.value.as_float())
            .map_or(false, |v| v > 0.0);

        let eq = Self::build_eq(
            Self::get_bandwidths(&parameters),
            Self::get_gains(&parameters),
            high_shelf_enabled,
            low_shelf_enabled
        );
        Ok(GraphicEq7 {
            parameters,
            response_plot: Self::amplitude_response_plot(&eq),
            eq,
            id: unique_time_id(),
        })
    }
}


impl GraphicEq7 {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        let init_gain = 0.0;
        let init_bandwidth = 1.05;
        for i in 0..7 {
            parameters.insert(
                format!("gain{}", i + 1),
                PedalParameter {
                    value: PedalParameterValue::Float(init_gain),
                    min: Some(PedalParameterValue::Float(-15.0)),
                    max: Some(PedalParameterValue::Float(15.0)),
                    step: Some(PedalParameterValue::Float(0.1))
                },
            );
            parameters.insert(
                format!("bandwidth{}", i + 1),
                PedalParameter {
                    value: PedalParameterValue::Float(init_bandwidth),
                    min: Some(PedalParameterValue::Float(0.1)),
                    max: Some(PedalParameterValue::Float(2.0)),
                    step: Some(PedalParameterValue::Float(0.01))
                },
            );
        }

        parameters.insert(
            "low_shelf".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: Some(PedalParameterValue::Float(1.0))
            },
        );

        parameters.insert(
            "high_shelf".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: Some(PedalParameterValue::Float(1.0))
            },
        );

        let eq = Self::build_eq([init_bandwidth; 7], [init_gain; 7], true, false);

        GraphicEq7 { parameters, eq, id: unique_time_id(), response_plot: Vec::new() }
    }

    pub fn amplitude_response_plot(eq: &Equalizer) -> Vec<PlotPoint> {
        eq.amplitude_response_plot(48000.0, 60.0, 15000.0, 100)
    }

    pub fn get_gains(parameters: &HashMap<String, PedalParameter>) -> [f32; 7] {
        [
            parameters.get("gain1").unwrap().value.as_float().unwrap(),
            parameters.get("gain2").unwrap().value.as_float().unwrap(),
            parameters.get("gain3").unwrap().value.as_float().unwrap(),
            parameters.get("gain4").unwrap().value.as_float().unwrap(),
            parameters.get("gain5").unwrap().value.as_float().unwrap(),
            parameters.get("gain6").unwrap().value.as_float().unwrap(),
            parameters.get("gain7").unwrap().value.as_float().unwrap(),
        ]
    }

    pub fn get_bandwidths(parameters: &HashMap<String, PedalParameter>) -> [f32; 7] {
        [
            parameters.get("bandwidth1").unwrap().value.as_float().unwrap(),
            parameters.get("bandwidth2").unwrap().value.as_float().unwrap(),
            parameters.get("bandwidth3").unwrap().value.as_float().unwrap(),
            parameters.get("bandwidth4").unwrap().value.as_float().unwrap(),
            parameters.get("bandwidth5").unwrap().value.as_float().unwrap(),
            parameters.get("bandwidth6").unwrap().value.as_float().unwrap(),
            parameters.get("bandwidth7").unwrap().value.as_float().unwrap(),
        ]
    }

    fn build_eq(bandwidths: [f32; 7], gains: [f32; 7], high_shelf: bool, low_shelf: bool) -> eq::Equalizer {
        let mut b = eq::GraphicEqualizerBuilder::new(48000.0)
            .with_bands([100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0])
            .with_bandwidths(bandwidths)
            .with_gains(gains);

        if high_shelf {
            b = b.with_upper_shelf()
        };

        if low_shelf {
            b = b.with_lower_shelf()
        };

        b.build()
    }
}


impl PedalTrait for GraphicEq7 {
    fn process_audio(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.eq.process(*sample);
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        if let Some(param) = self.parameters.get_mut(name) {
            if param.is_valid(&value) {
                param.value = value;

                if name.starts_with("gain") || name.starts_with("bandwidth") || name == "low_shelf" || name == "high_shelf" {
                    let low_shelf = self.parameters.get("low_shelf").unwrap().value.as_float().unwrap() > 0.0;
                    let high_shelf = self.parameters.get("high_shelf").unwrap().value.as_float().unwrap() > 0.0;
                    let gains = Self::get_gains(&self.parameters);
                    let bandwidths = Self::get_bandwidths(&self.parameters);
                    self.eq = Self::build_eq(bandwidths, gains, high_shelf, low_shelf);
                    self.response_plot = Self::amplitude_response_plot(&self.eq);
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui) -> Option<(String, PedalParameterValue)> {
        let mut changed_param = None;

        let pedal_size = ui.available_size();

        let mut img_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(ui.available_rect_before_wrap())
        );

        img_ui.add(Image::new(include_image!("images/pedal_gradient.png")).tint(Color32::from_rgb(208, 76, 40)));

        // Title row with shelf buttons
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::from_black_alpha(50);
            ui.style_mut().visuals.widgets.hovered.weak_bg_fill = Color32::from_black_alpha(80);
            ui.columns_const(|[col1, col2, col3]| {
                let high_shelf_enabled = self.parameters.get("high_shelf").unwrap().value.as_float().unwrap() > 0.0;
                let low_shelf_enabled = self.parameters.get("low_shelf").unwrap().value.as_float().unwrap() > 0.0;

                col1.centered_and_justified(|ui| {
                    if ui.add(
                        ImageButton::new(
                            Image::new(include_image!("images/eq/low_shelf.png")).max_width(pedal_size.x * 0.15)
                        ).corner_radius(3.0)
                        .selected(low_shelf_enabled)
                    ).on_hover_text("Low Shelf")
                    .clicked() {
                        let new_value = if low_shelf_enabled { 0.0 } else { 1.0 };
                        changed_param = Some(("low_shelf".to_string(), PedalParameterValue::Float(new_value)));
                    }
                });
                

                col2.centered_and_justified(|ui| ui.label("EQ"));

                col3.centered_and_justified(|ui| {
                    if ui.add(
                        ImageButton::new(
                            Image::new(include_image!("images/eq/high_shelf.png")).max_width(pedal_size.x * 0.15)
                        ).corner_radius(3.0)
                        .selected(high_shelf_enabled)
                    ).on_hover_text("High Shelf")
                    .clicked() {
                        let new_value = if high_shelf_enabled { 0.0 } else { 1.0 };
                        changed_param = Some(("high_shelf".to_string(), PedalParameterValue::Float(new_value)));
                    }
                });
            });
        });

        // Knobs for each band
        ui.add_space(2.0);
        ui.horizontal_top(|ui| {
            let width = pedal_size.x / 9.0;
            let spacing = pedal_size.x / 34.0;
            ui.spacing_mut().item_spacing = egui::Vec2::new(spacing, 0.0);
            ui.add_space(spacing/2.0);

            let mut changed_eq_param = None;
            if let Some(change) = eq_knob(ui, "100hz", self.parameters.get("gain1").unwrap(), self.parameters.get("bandwidth1").unwrap(), width) {
                changed_eq_param = Some((1, change));
            }
            if let Some(change) = eq_knob(ui, "200hz", self.parameters.get("gain2").unwrap(), self.parameters.get("bandwidth2").unwrap(), width) {
                changed_eq_param = Some((2, change));
            }
            if let Some(change) = eq_knob(ui, "400hz", self.parameters.get("gain3").unwrap(), self.parameters.get("bandwidth3").unwrap(), width) {
                changed_eq_param = Some((3, change));
            }
            if let Some(change) = eq_knob(ui, "800hz", self.parameters.get("gain4").unwrap(), self.parameters.get("bandwidth4").unwrap(), width) {
                changed_eq_param = Some((4, change));
            }
            if let Some(change) = eq_knob(ui, "1.6khz", self.parameters.get("gain5").unwrap(), self.parameters.get("bandwidth5").unwrap(), width) {
                changed_eq_param = Some((5, change));
            }
            if let Some(change) = eq_knob(ui, "3.2khz", self.parameters.get("gain6").unwrap(), self.parameters.get("bandwidth6").unwrap(), width) {
                changed_eq_param = Some((6, change));
            }
            if let Some(change) = eq_knob(ui, "6.4khz", self.parameters.get("gain7").unwrap(), self.parameters.get("bandwidth7").unwrap(), width) {
                changed_eq_param = Some((7, change));
            }

            if let Some((i, change)) = changed_eq_param {
                match change {
                    EqChange::Gain(value) => {
                        let param_name = format!("gain{}", i);
                        changed_param = Some((param_name, PedalParameterValue::Float(value)));
                    },
                    EqChange::Bandwidth(value) => {
                        let param_name = format!("bandwidth{}", i);
                        changed_param = Some((param_name, PedalParameterValue::Float(value)));
                    }
                }
            }
        });

        // Frequency Response Graph
        ui.add_space(2.0);

        Plot::new(self.id)
            .height(ui.available_height()*0.75)
            .width(ui.available_width())
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .show_x(false)
            .show_y(false)
            .center_y_axis(true)
            .show_axes(false)
            .include_y(15.0)
            .include_y(-15.0)
            .show_grid(false)
            .set_margin_fraction(Vec2::ZERO)
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new("freq_response", self.response_plot.as_slice())
                        .width(1.0)
                );

                let freqs: [f64; 7] = [100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0];
                for hz in freqs {
                    let log2_hz = hz.log2();
                    plot_ui.vline(
                        VLine::new("", log2_hz)
                            .color(Color32::DARK_GRAY)
                            .width(1.0)
                    );
                }

                plot_ui.hline(
                    HLine::new("", 0.0)
                        .color(Color32::GRAY)
                        .width(1.0)
                );
            });

        changed_param
    }
}

enum EqChange {
    Gain(f32),
    Bandwidth(f32),
}

fn eq_knob(ui: &mut eframe::egui::Ui, label: &str, param: &PedalParameter, bandwidth_param: &PedalParameter, width: f32) -> Option<EqChange> {
    ui.vertical(|ui| {
        let mut changed_param = None;

        let slot = Image::new(include_image!("images/eq/slot.png"))
            .max_width(width);
        let slot_response = ui.add(slot);

        let knob = Image::new(include_image!("images/eq/knob.png"))
            .max_width(width)
            .sense(egui::Sense::click_and_drag());

        let knob_height = knob.load_and_calc_size(ui, slot_response.rect.size()).expect("File image should load").y;
        let param_max = param.max.as_ref().unwrap().as_float().unwrap();
        let param_min = param.min.as_ref().unwrap().as_float().unwrap();
        let param_value = param.value.as_float().unwrap();
        let knob_frac = 1.0 - (param_value - param_min) / (param_max - param_min);
        let knob_y_offset = slot_response.rect.height() * knob_frac;
        let knob_rect = slot_response.rect
            .translate(egui::Vec2::new(0.0, knob_y_offset-knob_height/2.0));
        let knob_response = ui.new_child(
            UiBuilder::new()
                .max_rect(knob_rect)
                .layout(egui::Layout::top_down(egui::Align::Center))
                .sense(egui::Sense::click_and_drag()),
        ).add(knob);

        if knob_response.hovered() {
            ui.ctx().output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeVertical);
        }

        if knob_response.dragged() {
            let delta = -knob_response.drag_delta().y / slot_response.rect.height();
            let new_value = param_value + delta * (param_max - param_min);
            let clamped_value = new_value.clamp(param_min, param_max);
            changed_param = Some(EqChange::Gain(clamped_value));
        }

        ui.label(RichText::new(label).size(3.0));

        // Allocate space for bandwidth knob
        // Using allocate_ui_with_layout doesn't seem to allocate the correct space so this is a hack
        let (_, knob_rect) = ui.allocate_space(Vec2::splat(width));
        let mut bandwidth_knob_ui = ui.new_child(
            UiBuilder::new().max_rect(knob_rect)
        );

        if let Some(value) = pedal_knob(&mut bandwidth_knob_ui, "", bandwidth_param, Vec2::ZERO, 1.0) {
            changed_param = Some(EqChange::Bandwidth(value.as_float().unwrap()));
        }

        changed_param
    }).inner
}
