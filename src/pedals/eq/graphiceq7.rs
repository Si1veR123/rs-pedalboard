use std::hash::Hash;
use std::{collections::HashMap, time::Instant};

use eframe::egui::{self, include_image, Button, Color32, Image, UiBuilder, Vec2};
use egui_plot::{HLine, Line, Plot, PlotPoint, VLine};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};

use super::super::{PedalParameter, PedalParameterValue, PedalTrait};
use super::frequency_plot::{self, LIVE_FREQUENCY_COLOR};
use super::{
    deserialize_plot_points, gain_knob, serialize_plot_points, EQ_DB_GAIN,
    MAX_FREQ_HZ, MIN_FREQ_HZ, PLOT_POINTS, SLOT_ASPECT_RATIO,
    get_band_image_from_value, get_band_type_from_value, get_band_type_next_value
};

use crate::pedalboard::ParameterPath;
use crate::{
    dsp_algorithms::{
        eq::{self, Equalizer},
        frequency_analysis::FrequencyAnalyser,
    },
    pedals::ui::pedal_knob,
    unique_time_id, DEFAULT_REFRESH_DURATION,
};

/// The centre frequency of each of the pedal's seven bands, marked with a grey bar down its graph
/// and built into the filters its knobs edit.
///
/// These are the pedal's own bands, which its face has a knob pair for each of. The global EQ covers
/// the same range with ten bands a little under an octave apart, which are its own rather than the
/// pedal's.
const BAND_FREQS: [f32; 7] = [80.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0];

const LIVE_FREQUENCY_UPDATE_MS: usize = 100;
const OVERSAMPLE: f32 = 10.0; // Oversample for frequency analysis

pub struct GraphicEq7 {
    parameters: HashMap<String, PedalParameter>,
    eq: eq::Equalizer,
    sample_rate: f32,
    id: u32,
    response_plot: Vec<PlotPoint>,

    // Only exists on processor
    frequency_analyser: Option<FrequencyAnalyser>,
    last_frequencies_sent: Instant,

    // Used for smoothing the frequency plot
    prev_live_frequency_plot: Vec<PlotPoint>,
    target_live_frequency_plot: Vec<PlotPoint>,
    last_frame: Instant,
}

impl Clone for GraphicEq7 {
    fn clone(&self) -> Self {
        GraphicEq7 {
            parameters: self.parameters.clone(),
            eq: self.eq.clone(),
            sample_rate: self.sample_rate,
            id: self.id,
            response_plot: self.response_plot.clone(),
            frequency_analyser: None,
            last_frequencies_sent: Instant::now(),
            prev_live_frequency_plot: Vec::with_capacity(PLOT_POINTS),
            target_live_frequency_plot: Vec::with_capacity(PLOT_POINTS),
            last_frame: Instant::now(),
        }
    }
}

impl Hash for GraphicEq7 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Serialize for GraphicEq7 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_map = serializer.serialize_map(Some(2))?;
        ser_map.serialize_entry("id", &self.id)?;
        ser_map.serialize_entry("parameters", &self.parameters)?;
        ser_map.end()
    }
}

impl<'a> Deserialize<'a> for GraphicEq7 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        #[derive(Deserialize)]
        struct GraphicEq7Data {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }

        let helper = GraphicEq7Data::deserialize(deserializer)?;

        let mut parameters = helper.parameters;

        // Set live_frequency_plot to false when loading the pedal as it is intensive
        parameters
            .entry("Live Frequency Plot".to_string())
            .and_modify(|p| p.value = PedalParameterValue::Bool(false))
            .or_insert_with(|| PedalParameter {
                value: PedalParameterValue::Bool(false),
                min: None,
                max: None,
                step: None,
            });

        let high_band_mode = parameters
            .get("High Band Mode")
            .and_then(|p| p.value.as_int())
            .unwrap_or(0);

        let low_band_mode = parameters
            .get("Low Band Mode")
            .and_then(|p| p.value.as_int())
            .unwrap_or(0);

        let eq = Self::build_eq(
            Self::get_bandwidths(&parameters),
            Self::get_gains(&parameters),
            high_band_mode,
            low_band_mode,
            48000.0,
        );
        Ok(GraphicEq7 {
            parameters,
            response_plot: Self::amplitude_response_plot(&eq, 48000.0),
            eq,
            sample_rate: 48000.0, // Default sample rate, can be set later
            id: helper.id,
            prev_live_frequency_plot: Vec::with_capacity(PLOT_POINTS),
            target_live_frequency_plot: Vec::with_capacity(PLOT_POINTS),
            last_frame: Instant::now(),
            frequency_analyser: None,
            last_frequencies_sent: Instant::now(),
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
                format!("Gain {}", i + 1),
                PedalParameter {
                    value: PedalParameterValue::Float(init_gain),
                    min: Some(PedalParameterValue::Float(-EQ_DB_GAIN)),
                    max: Some(PedalParameterValue::Float(EQ_DB_GAIN)),
                    step: Some(PedalParameterValue::Float(0.1)),
                },
            );
            parameters.insert(
                format!("Bandwidth {}", i + 1),
                PedalParameter {
                    value: PedalParameterValue::Float(init_bandwidth),
                    min: Some(PedalParameterValue::Float(0.1)),
                    max: Some(PedalParameterValue::Float(2.0)),
                    step: Some(PedalParameterValue::Float(0.01)),
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
            "Live Frequency Plot".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(false),
                min: None,
                max: None,
                step: None,
            },
        );

        parameters.insert(
            "Dry/Wet".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
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

        let eq = Self::build_eq([init_bandwidth; 7], [init_gain; 7], 0, 0, 48000.0);

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
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }

    pub fn frequency_analyser(sample_rate: f32) -> FrequencyAnalyser {
        FrequencyAnalyser::new(
            sample_rate,
            MIN_FREQ_HZ,
            MAX_FREQ_HZ,
            PLOT_POINTS,
            OVERSAMPLE,
        )
    }

    pub fn amplitude_response_plot(eq: &Equalizer, sample_rate: f32) -> Vec<PlotPoint> {
        eq.amplitude_response_plot(
            sample_rate as f64,
            MIN_FREQ_HZ as f64,
            MAX_FREQ_HZ as f64,
            PLOT_POINTS,
        )
    }

    pub fn get_gains(parameters: &HashMap<String, PedalParameter>) -> [f32; 7] {
        [
            parameters.get("Gain 1").unwrap().value.as_float().unwrap(),
            parameters.get("Gain 2").unwrap().value.as_float().unwrap(),
            parameters.get("Gain 3").unwrap().value.as_float().unwrap(),
            parameters.get("Gain 4").unwrap().value.as_float().unwrap(),
            parameters.get("Gain 5").unwrap().value.as_float().unwrap(),
            parameters.get("Gain 6").unwrap().value.as_float().unwrap(),
            parameters.get("Gain 7").unwrap().value.as_float().unwrap(),
        ]
    }

    pub fn get_bandwidths(parameters: &HashMap<String, PedalParameter>) -> [f32; 7] {
        [
            parameters
                .get("Bandwidth 1")
                .unwrap()
                .value
                .as_float()
                .unwrap(),
            parameters
                .get("Bandwidth 2")
                .unwrap()
                .value
                .as_float()
                .unwrap(),
            parameters
                .get("Bandwidth 3")
                .unwrap()
                .value
                .as_float()
                .unwrap(),
            parameters
                .get("Bandwidth 4")
                .unwrap()
                .value
                .as_float()
                .unwrap(),
            parameters
                .get("Bandwidth 5")
                .unwrap()
                .value
                .as_float()
                .unwrap(),
            parameters
                .get("Bandwidth 6")
                .unwrap()
                .value
                .as_float()
                .unwrap(),
            parameters
                .get("Bandwidth 7")
                .unwrap()
                .value
                .as_float()
                .unwrap(),
        ]
    }

    fn build_eq(
        bandwidths: [f32; 7],
        gains: [f32; 7],
        high_band_mode: i16,
        low_band_mode: i16,
        sample_rate: f32,
    ) -> eq::Equalizer {
        let mut b = eq::GraphicEqualizerBuilder::new(sample_rate)
            .with_bands(BAND_FREQS)
            .with_bandwidths(bandwidths)
            .with_gains(gains);

        if let Some(band_type) = get_band_type_from_value(high_band_mode) {
            b = b.with_upper_band_type(band_type);
        }

        if let Some(band_type) = get_band_type_from_value(low_band_mode) {
            b = b.with_lower_band_type(band_type);
        }

        b.build()
    }
}

impl PedalTrait for GraphicEq7 {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn process_audio(&mut self, buffer: &mut [f32], message_buffer: &mut Vec<String>) {
        let dry_wet = self
            .parameters
            .get("Dry/Wet")
            .unwrap()
            .value
            .as_float()
            .unwrap();
        for sample in buffer.iter_mut() {
            *sample = self.eq.process(*sample) * dry_wet + *sample * (1.0 - dry_wet);
        }

        if self
            .parameters
            .get("Live Frequency Plot")
            .unwrap()
            .value
            .as_bool()
            .unwrap()
        {
            let frequency_analyser = self
                .frequency_analyser
                .as_mut()
                .expect("Frequency Analyser should not be None on processor");
            frequency_analyser.push_samples(buffer);

            // Check if enough time has passed since the last update
            if self.last_frequencies_sent.elapsed().as_millis() as usize >= LIVE_FREQUENCY_UPDATE_MS
            {
                if frequency_analyser.analyse_log2(&mut self.target_live_frequency_plot) {
                    // New frequency data available, serialize and send to client
                    self.last_frequencies_sent = Instant::now();
                    let message = serialize_plot_points(&mut self.target_live_frequency_plot);
                    message_buffer.push(message);
                }
            }
        }
    }

    fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
        self.sample_rate = sample_rate as f32;
        if self.frequency_analyser.is_none() {
            self.frequency_analyser = Some(Self::frequency_analyser(self.sample_rate));
        }

        let high_band_mode = self
            .parameters
            .get("High Band Mode")
            .map(|p| p.value.as_int().unwrap_or(0))
            .unwrap_or(0);
        let low_band_mode = self
            .parameters
            .get("Low Band Mode")
            .map(|p| p.value.as_int().unwrap_or(0))
            .unwrap_or(0);

        self.eq = Self::build_eq(
            Self::get_bandwidths(&self.parameters),
            Self::get_gains(&self.parameters),
            high_band_mode,
            low_band_mode,
            self.sample_rate,
        );
    }

    fn parameter_value_display_string(
        &self,
        name: &str,
        value: &PedalParameterValue,
    ) -> Option<String> {
        if name.starts_with("Gain ") || name.starts_with("Bandwidth ") {
            Some(format!("{:.2}", value.as_float().unwrap()))
        } else if name == "Low Band Mode" || name == "High Band Mode" {
            match value.as_int().unwrap() {
                0 => Some("Peaking".to_string()),
                1 => Some("Shelf".to_string()),
                2 => Some("Pass".to_string()),
                _ => None,
            }
        } else if name == "Dry/Wet" {
            Some(format!("{:.0}%", value.as_float().unwrap() * 100.0))
        } else {
            None
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

                if name.starts_with("Gain ")
                    || name.starts_with("Bandwidth ")
                    || name == "Low Band Mode"
                    || name == "High Band Mode"
                {
                    let low_band_mode = self
                        .parameters
                        .get("Low Band Mode")
                        .map(|p| p.value.as_int().unwrap_or(0))
                        .unwrap_or(0);
                    let high_band_mode = self
                        .parameters
                        .get("High Band Mode")
                        .map(|p| p.value.as_int().unwrap_or(0))
                        .unwrap_or(0);

                    let gains = Self::get_gains(&self.parameters);
                    let bandwidths = Self::get_bandwidths(&self.parameters);
                    self.eq = Self::build_eq(bandwidths, gains, high_band_mode, low_band_mode, self.sample_rate);
                    self.response_plot = Self::amplitude_response_plot(&self.eq, self.sample_rate);
                }
            }
        }
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        message_buffer: &[String],
    ) -> Option<(String, PedalParameterValue)> {
        let live_frequency_enabled = self
            .parameters
            .get("Live Frequency Plot")
            .unwrap()
            .value
            .as_bool()
            .unwrap();
        if message_buffer.len() > 0 {
            // Deserialize the frequency response plot from the message buffer
            if let Ok(mut plot_points) = deserialize_plot_points(&message_buffer[0]) {
                frequency_plot::payload_to_plot_points(&mut plot_points, EQ_DB_GAIN as f64);
                self.target_live_frequency_plot = plot_points;
            } else {
                tracing::error!("Failed to deserialize frequency response plot");
            }
        }

        if live_frequency_enabled {
            // Ease the plotted curve towards the newest measurements
            frequency_plot::update_live_frequency_plot(
                &mut self.prev_live_frequency_plot,
                &self.target_live_frequency_plot,
                self.last_frame.elapsed().as_secs_f64() * 1000.0,
            );
            self.last_frame = Instant::now();

            ui.ctx().request_repaint_after(DEFAULT_REFRESH_DURATION);
        }

        let mut changed_param = None;

        let pedal_size = ui.available_size();

        let rect = ui.available_rect_before_wrap();
        ui.new_child(UiBuilder::new().max_rect(rect))
            .add(Image::new(include_image!("../images/eq_new.png")));

        let low_band_mode = self
            .parameters
            .get("Low Band Mode")
            .unwrap()
            .value
            .as_int()
            .unwrap();

        let high_band_mode = self
            .parameters
            .get("High Band Mode")
            .unwrap()
            .value
            .as_int()
            .unwrap();

        // Title row with shelf buttons
        ui.add_space(2.0);
        let top_section_height = pedal_size.y * 0.075;
        ui.allocate_ui_with_layout(Vec2::new(pedal_size.x, top_section_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::from_black_alpha(50);
            ui.style_mut().visuals.widgets.hovered.weak_bg_fill = Color32::from_black_alpha(80);
            ui.columns_const(|[col1, _col2, col3]| {
                col1.centered_and_justified(|ui| {
                    if ui
                        .add(
                            Button::image(
                                Image::new(get_band_image_from_value(low_band_mode))
                                    .max_width(pedal_size.x * 0.15),
                            )
                            .corner_radius(3.0),
                        )
                        .on_hover_text("Low Band Mode")
                        .clicked()
                    {
                        changed_param = Some((
                            "Low Band Mode".to_string(),
                            PedalParameterValue::Int(get_band_type_next_value(low_band_mode)),
                        ));
                    }
                });

                col3.centered_and_justified(|ui| {
                    if ui
                        .add(
                            Button::image(
                                Image::new(get_band_image_from_value(high_band_mode))
                                    .max_width(pedal_size.x * 0.15),
                            )
                            .corner_radius(3.0),
                        )
                        .on_hover_text("High Band Mode")
                        .clicked()
                    {
                        changed_param = Some((
                            "High Band Mode".to_string(),
                            PedalParameterValue::Int(get_band_type_next_value(high_band_mode)),
                        ));
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
            ui.add_space(spacing / 2.0);

            let mut changed_eq_param = None;

            for i in 1..=7 {
                // Low/High pass filters dont have a gain knob
                let bandwidth_only = (i == 1 && low_band_mode == 2) || (i == 7 && high_band_mode == 2);

                if let Some(change) = eq_knob(
                    ui,
                    self.parameters.get(&format!("Gain {}", i)).unwrap(),
                    self.parameters.get(&format!("Bandwidth {}", i)).unwrap(),
                    i,
                    width,
                    self.id,
                    bandwidth_only
                ) {
                    changed_eq_param = Some((i, change));
                }
            }

            if let Some((i, change)) = changed_eq_param {
                match change {
                    EqChange::Gain(value) => {
                        let param_name = format!("Gain {}", i);
                        changed_param = Some((param_name, PedalParameterValue::Float(value)));
                    }
                    EqChange::Bandwidth(value) => {
                        let param_name = format!("Bandwidth {}", i);
                        changed_param = Some((param_name, PedalParameterValue::Float(value)));
                    }
                }
            }
        });

        // Frequency Response Graph
        ui.add_space(pedal_size.y * 0.03);

        let plot_response = Plot::new(self.id)
            .height(ui.available_height() * 0.75)
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
                if live_frequency_enabled {
                    // Drawn behind the response curve, with a translucent fill, so the curve
                    // the knobs actually edit stays the clearest thing in the graph
                    let fill_bottom = frequency_plot::live_plot_fill_bottom(
                        &self.response_plot,
                        &self.prev_live_frequency_plot,
                        EQ_DB_GAIN as f64,
                    );

                    plot_ui.line(
                        Line::new("live_frequency", self.prev_live_frequency_plot.as_slice())
                            .color(LIVE_FREQUENCY_COLOR)
                            .width(1.0_f32)
                            .fill(fill_bottom as f32)
                            .fill_alpha(0.15),
                    );
                }

                plot_ui.line(
                    Line::new("freq_response", self.response_plot.as_slice())
                        .width(1.0_f32)
                        .color(Color32::from_rgb(150, 150, 245)),
                );

                for hz in BAND_FREQS {
                    plot_ui.vline(
                        VLine::new("", (hz as f64).log2())
                            .color(Color32::DARK_GRAY)
                            .width(1.0_f32),
                    );
                }

                plot_ui.hline(HLine::new("", 0.0).color(Color32::GRAY).width(1.0_f32));
            });

        let mut live_freq_response_button_rect =
            plot_response.response.rect.translate(Vec2::splat(-2.0));
        live_freq_response_button_rect.min = live_freq_response_button_rect.max - Vec2::splat(13.0);

        let live_frequency_tint = if live_frequency_enabled {
            LIVE_FREQUENCY_COLOR
        } else {
            Color32::from_gray(110)
        };

        let live_frequency_response = ui
            .new_child(
                UiBuilder::new()
                    .max_rect(live_freq_response_button_rect)
                    .sense(egui::Sense::click()),
            )
            .add(
                Button::image(
                    Image::new(include_image!("../images/eq/live.png")).tint(live_frequency_tint),
                )
                .corner_radius(3.0)
                // Kept for accessibility, the outline below is what is actually visible
                .selected(live_frequency_enabled)
                .frame(false),
            );

        if live_frequency_enabled {
            ui.painter().rect_stroke(
                live_frequency_response.rect,
                3.0,
                (1.5, LIVE_FREQUENCY_COLOR),
                egui::StrokeKind::Inside,
            );
        }

        if live_frequency_response.clicked() {
            changed_param = Some((
                "Live Frequency Plot".to_string(),
                PedalParameterValue::Bool(!live_frequency_enabled),
            ));
        }

        changed_param
    }
}

enum EqChange {
    Gain(f32),
    Bandwidth(f32),
}

fn eq_knob(
    ui: &mut eframe::egui::Ui,
    param: &PedalParameter,
    bandwidth_param: &PedalParameter,
    param_num: usize,
    width: f32,
    pedal_id: u32,
    bandwidth_only: bool
) -> Option<EqChange> {
    let parameter_path = ParameterPath {
        pedalboard_id: None,
        pedal_id,
        parameter_name: format!("Gain {param_num}"),
    };

    ui.vertical(|ui| {
        let mut changed_param = None;

        if bandwidth_only {
            // Allocate space for gain knob but don't draw it
            ui.allocate_space(Vec2::new(width, width * SLOT_ASPECT_RATIO));
        } else {
            // The slot is drawn at the size of the range a band's gain is set over, so the knob travels
            // over the whole of it
            if let Some(value) = gain_knob(
                ui,
                param,
                Vec2::new(width, width * SLOT_ASPECT_RATIO),
                width,
                1.0,
                Some(&parameter_path)
            ) {
                changed_param = Some(EqChange::Gain(value));
            }
        }

        ui.add_space(2.0);

        // Allocate space for bandwidth knob
        // Using allocate_ui_with_layout doesn't seem to allocate the correct space so this is a hack
        let (_, knob_rect) = ui.allocate_space(Vec2::splat(width));
        let mut bandwidth_knob_ui = ui.new_child(UiBuilder::new().max_rect(knob_rect));

        if let Some(value) = pedal_knob(
            &mut bandwidth_knob_ui,
            "",
            &format!("Bandwidth {param_num}"),
            bandwidth_param,
            Vec2::ZERO,
            1.0,
            pedal_id,
        ) {
            changed_param = Some(EqChange::Bandwidth(value.as_float().unwrap()));
        }

        changed_param
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The response curve the graph is drawn with for a set of gains.
    fn response(gains: [f32; 7], bandwidths: [f32; 7]) -> Vec<PlotPoint> {
        let eq = GraphicEq7::build_eq(bandwidths, gains, 0, 0, 48000.0);

        GraphicEq7::amplitude_response_plot(&eq, 48000.0)
    }

    #[test]
    fn the_fill_reaches_the_bottom_of_a_graph_the_response_grows() {
        // Summed bands reach well past the range a single knob is allowed, which is what
        // takes the bottom edge of the graph down with them ...
        let boosted_response = response([EQ_DB_GAIN; 7], [1.05; 7]);
        let tallest = boosted_response
            .iter()
            .map(|point| point.y.abs())
            .fold(0.0_f64, f64::max);

        assert!(tallest > EQ_DB_GAIN as f64, "tallest: {tallest}");

        // ... so the fill is asked for the graph's own bottom edge rather than the mapped
        // floor, which used to leave the shading hanging above it
        assert_eq!(
            frequency_plot::live_plot_fill_bottom(&boosted_response, &[], EQ_DB_GAIN as f64),
            -tallest
        );

        // A response that fits inside the knobs' range keeps the graph, and so the fill, at
        // the window the live curve is mapped over
        assert_eq!(
            frequency_plot::live_plot_fill_bottom(
                &response([0.0; 7], [1.05; 7]),
                &[],
                EQ_DB_GAIN as f64
            ),
            -(EQ_DB_GAIN as f64)
        );
    }
}
