use std::{collections::HashMap, time::Instant};
use std::hash::Hash;

use eframe::egui::{self, include_image, Color32, Image, ImageButton, UiBuilder, Vec2};
use egui_plot::{HLine, Line, Plot, PlotPoint, VLine};
use serde::{Deserialize, Serialize};

use super::{PedalParameter, PedalParameterValue, PedalTrait};

use crate::{dsp_algorithms::{eq::{self, Equalizer}, frequency_analysis::FrequencyAnalyser}, pedals::ui::pedal_knob, unique_time_id, DEFAULT_REFRESH_DURATION};

const PLOT_POINTS: usize = 80;
const LIVE_FREQUENCY_UPDATE_MS: usize = 100;
const EQ_DB_GAIN: f32 = 15.0;
const OVERSAMPLE: f32 = 10.0;

pub fn serialize_plot_points(plot_points: &mut [PlotPoint]) -> String {
    // First round the points to 2 decimal places to reduce size
    for point in plot_points.iter_mut() {
        point.x = (point.x * 100.0).round() / 100.0;
        point.y = (point.y * 100.0).round() / 100.0;
    }

    // Hoping the compiler will optimise this
    let plot_points_floats: Vec<[f64; 2]> = plot_points.iter()
        .map(|p| [p.x, p.y])
        .collect();

    serde_json::to_string(&plot_points_floats).expect("Failed to serialize plot points")
}

pub fn deserialize_plot_points(data: &str) -> serde_json::Result<Vec<PlotPoint>> {
    let plot_points_floats: Vec<[f64; 2]> = serde_json::from_str(data)?;

    // Hoping the compiler will optimise this
    let plot_points = plot_points_floats.into_iter()
        .map(|p| PlotPoint::new(p[0], p[1]))
        .collect::<Vec<PlotPoint>>();

    Ok(plot_points)
}

#[derive(Clone)]
pub struct GraphicEq7 {
    parameters: HashMap<String, PedalParameter>,
    eq: eq::Equalizer,
    sample_rate: f32,
    id: usize,
    response_plot: Vec<PlotPoint>,

    // Only exists on server
    frequency_analyser: Option<FrequencyAnalyser>,
    last_frequencies_sent: Instant,

    // Used for smoothing the frequency plot
    prev_live_frequency_plot: Vec<PlotPoint>,
    target_live_frequency_plot: Vec<PlotPoint>,
    last_frame: Instant,

    // Used to clamp the live frequency plot values
    dynamic_max: f32
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
        let mut parameters: HashMap<String, PedalParameter> = HashMap::deserialize(deserializer)?;

        // Set live_frequency_plot to 0 when loading the pedal as it is intensive
        parameters.entry("live_frequency_plot".to_string())
            .and_modify(|p| p.value = PedalParameterValue::Bool(false))
            .or_insert_with(|| PedalParameter {
                value: PedalParameterValue::Bool(false),
                min: None,
                max: None,
                step: None
            });

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
            low_shelf_enabled,
            48000.0
        );
        Ok(GraphicEq7 {
            parameters,
            response_plot: Self::amplitude_response_plot(&eq, 48000.0),
            eq,
            sample_rate: 48000.0, // Default sample rate, can be set later
            id: unique_time_id(),
            prev_live_frequency_plot: Vec::with_capacity(PLOT_POINTS),
            target_live_frequency_plot: Vec::with_capacity(PLOT_POINTS),
            last_frame: Instant::now(),
            frequency_analyser: None,
            last_frequencies_sent: Instant::now(),
            dynamic_max: 0.0
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
                    min: Some(PedalParameterValue::Float(-EQ_DB_GAIN)),
                    max: Some(PedalParameterValue::Float(EQ_DB_GAIN)),
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

        parameters.insert(
            "live_frequency_plot".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(false),
                min: None,
                max: None,
                step: None
            },
        );

        parameters.insert(
            "dry_wet".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None
            },
        );

        parameters.insert(
            "active".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None,
            },
        );

        let eq = Self::build_eq([init_bandwidth; 7], [init_gain; 7], true, false, 48000.0);

        GraphicEq7 {
            response_plot: Self::amplitude_response_plot(&eq, 48000.0),
            id: unique_time_id(),
            parameters,
            eq,
            sample_rate: 48000.0, // Default sample rate, can be set later
            prev_live_frequency_plot: Vec::with_capacity(PLOT_POINTS),
            target_live_frequency_plot: Vec::with_capacity(PLOT_POINTS),
            last_frame: Instant::now(),
            frequency_analyser: None,
            last_frequencies_sent: Instant::now(),
            dynamic_max: 0.0
        }
    }

    pub fn frequency_analyser(sample_rate: f32) -> FrequencyAnalyser {
        FrequencyAnalyser::new(sample_rate, 60.0, 11000.0, PLOT_POINTS, OVERSAMPLE)
    }

    pub fn amplitude_response_plot(eq: &Equalizer, sample_rate: f32) -> Vec<PlotPoint> {
        eq.amplitude_response_plot(sample_rate as f64, 60.0, 11000.0, PLOT_POINTS)
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

    fn build_eq(bandwidths: [f32; 7], gains: [f32; 7], high_shelf: bool, low_shelf: bool, sample_rate: f32) -> eq::Equalizer {
        let mut b = eq::GraphicEqualizerBuilder::new(sample_rate)
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
    fn process_audio(&mut self, buffer: &mut [f32], message_buffer: &mut Vec<String>) {
        let dry_wet = self.parameters.get("dry_wet").unwrap().value.as_float().unwrap();
        for sample in buffer.iter_mut() {
            *sample = self.eq.process(*sample) * dry_wet + *sample * (1.0 - dry_wet);
        }

        if self.parameters.get("live_frequency_plot").unwrap().value.as_bool().unwrap() {
            let frequency_analyser = self.frequency_analyser.as_mut().expect("Frequency Analyser should not be None on server");
            frequency_analyser.push_samples(buffer);

            // Check if enough time has passed since the last update
            if self.last_frequencies_sent.elapsed().as_millis() as usize >= LIVE_FREQUENCY_UPDATE_MS {
                if frequency_analyser.analyse_log2(&mut self.target_live_frequency_plot) {
                    // New frequency data available, serialize and send to client
                    self.last_frequencies_sent = Instant::now();
                    let message = serialize_plot_points(&mut self.target_live_frequency_plot);
                    message_buffer.push(message);
                }
            }
        }
    }

    fn set_config(&mut self, _buffer_size:usize, sample_rate:u32) {
        self.sample_rate = sample_rate as f32;
        if self.frequency_analyser.is_none() {
            self.frequency_analyser = Some(Self::frequency_analyser(self.sample_rate));
        }
        self.eq = Self::build_eq(
            Self::get_bandwidths(&self.parameters),
            Self::get_gains(&self.parameters),
            self.parameters.get("high_shelf").unwrap().value.as_float().unwrap() > 0.0,
            self.parameters.get("low_shelf").unwrap().value.as_float().unwrap() > 0.0,
            self.sample_rate
        );
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
                    self.eq = Self::build_eq(bandwidths, gains, high_shelf, low_shelf, self.sample_rate);
                    self.response_plot = Self::amplitude_response_plot(&self.eq, self.sample_rate);
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
        let live_frequency_enabled = self.parameters.get("live_frequency_plot").unwrap().value.as_bool().unwrap();
        if live_frequency_enabled {
            // Update the live frequency plot smoothly
            if self.prev_live_frequency_plot.is_empty() {
                self.prev_live_frequency_plot = self.target_live_frequency_plot.clone();
            }

            let time_since_last_frame_ms = self.last_frame.elapsed().as_millis() as usize;
            let smooth_factor = (time_since_last_frame_ms as f64 / LIVE_FREQUENCY_UPDATE_MS as f64).min(1.0);
            self.last_frame = Instant::now();

            for (prev, target) in self.prev_live_frequency_plot.iter_mut().zip(self.target_live_frequency_plot.iter()) {
                prev.x = prev.x * (1.0 - smooth_factor) + target.x * smooth_factor;
                prev.y = prev.y * (1.0 - smooth_factor) + target.y * smooth_factor;
            }

            ui.ctx().request_repaint_after(DEFAULT_REFRESH_DURATION);
        }

        if message_buffer.len() > 0 {
            // Deserialize the frequency response plot from the message buffer
            if let Ok(mut plot_points) = deserialize_plot_points(&message_buffer[0]) {
                // Scale plot points to 0-EQ_DB_GAIN
                let max_value = plot_points.iter()
                    .map(|p| p.y)
                    .fold(f64::NEG_INFINITY, |a, b| a.max(b));

                // Smoothly adjust dynamic max
                self.dynamic_max = (self.dynamic_max*0.9).max(max_value as f32);

                let scale_factor = EQ_DB_GAIN / self.dynamic_max as f32;
                for point in plot_points.iter_mut() {
                    point.y *= scale_factor as f64;
                }

                self.target_live_frequency_plot = plot_points;
            } else {
                log::error!("Failed to deserialize frequency response plot");
            }
        }

        let mut changed_param = None;

        let pedal_size = ui.available_size();

        let mut img_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(ui.available_rect_before_wrap())
        );

        img_ui.add(Image::new(include_image!("images/eq.png")));

        // Title row with shelf buttons
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::from_black_alpha(50);
            ui.style_mut().visuals.widgets.hovered.weak_bg_fill = Color32::from_black_alpha(80);
            ui.columns_const(|[col1, _col2, col3]| {
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
            if let Some(change) = eq_knob(ui, self.parameters.get("gain1").unwrap(), self.parameters.get("bandwidth1").unwrap(), width) {
                changed_eq_param = Some((1, change));
            }
            if let Some(change) = eq_knob(ui, self.parameters.get("gain2").unwrap(), self.parameters.get("bandwidth2").unwrap(), width) {
                changed_eq_param = Some((2, change));
            }
            if let Some(change) = eq_knob(ui, self.parameters.get("gain3").unwrap(), self.parameters.get("bandwidth3").unwrap(), width) {
                changed_eq_param = Some((3, change));
            }
            if let Some(change) = eq_knob(ui, self.parameters.get("gain4").unwrap(), self.parameters.get("bandwidth4").unwrap(), width) {
                changed_eq_param = Some((4, change));
            }
            if let Some(change) = eq_knob(ui, self.parameters.get("gain5").unwrap(), self.parameters.get("bandwidth5").unwrap(), width) {
                changed_eq_param = Some((5, change));
            }
            if let Some(change) = eq_knob(ui, self.parameters.get("gain6").unwrap(), self.parameters.get("bandwidth6").unwrap(), width) {
                changed_eq_param = Some((6, change));
            }
            if let Some(change) = eq_knob(ui, self.parameters.get("gain7").unwrap(), self.parameters.get("bandwidth7").unwrap(), width) {
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

        let plot_response = Plot::new(self.id)
            .height(ui.available_height()*0.75)
            .width(ui.available_width())
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .show_x(false)
            .show_y(false)
            .center_y_axis(true)
            .show_axes(false)
            .include_y(EQ_DB_GAIN)
            .include_y(-EQ_DB_GAIN)
            .show_grid(false)
            .set_margin_fraction(Vec2::ZERO)
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new("freq_response", self.response_plot.as_slice())
                        .width(1.0)
                        .color(Color32::from_rgb(150, 150, 245))
                );

                if self.parameters.get("live_frequency_plot").unwrap().value.as_bool().unwrap() {
                    plot_ui.line(
                        Line::new("live_frequency", self.prev_live_frequency_plot.as_slice())
                            .color(Color32::from_rgb(200, 0, 0))
                            .width(1.0)
                    );
                }

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

        let mut live_freq_response_button_rect = plot_response.response.rect.translate(Vec2::splat(-2.0));
        live_freq_response_button_rect.min = live_freq_response_button_rect.max - Vec2::splat(13.0);
        if ui.new_child(
            UiBuilder::new()
                .max_rect(live_freq_response_button_rect)
                .sense(egui::Sense::click())
        ).add(
            ImageButton::new(include_image!("images/eq/live.png"))
                .corner_radius(3.0)
                .tint(Color32::from_rgba_unmultiplied(220, 100, 100, 200))
                .selected(live_frequency_enabled)
                .frame(false)
        ).clicked() {
            changed_param = Some(("live_frequency_plot".to_string(), PedalParameterValue::Bool(!live_frequency_enabled)));
        }

        changed_param
    }


}

enum EqChange {
    Gain(f32),
    Bandwidth(f32),
}

fn eq_knob(ui: &mut eframe::egui::Ui, param: &PedalParameter, bandwidth_param: &PedalParameter, width: f32) -> Option<EqChange> {
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

        ui.add_space(7.0);

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