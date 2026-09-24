//! A graphic EQ that is applied to the whole signal chain rather than to a pedal on a pedalboard.
//!
//! Its ten bands are a little under an octave apart, set out evenly across the response graph with
//! half a band's share of the range past the band at either end, and the band at either end of the
//! range is a shelf, so a band at either end shapes everything past it. The graphic EQ pedal has
//! bands of its own, seven of them, so the two are not set out the same way.
//!
//! The settings live with the client's saved settings, so EQs can be created, renamed, deleted and
//! selected without a processor being connected, and the EQ that is applied can be sent to a
//! processor as soon as one is available.

use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, UiBuilder, Vec2};
use egui_plot::{HLine, Line, Plot, PlotPoint, VLine};
use serde::{Deserialize, Serialize};

use super::{frequency_plot, gain_knob, SLOT_ASPECT_RATIO};
use crate::{
    dsp_algorithms::eq::{self, Equalizer},
    pedals::{PedalParameter, PedalParameterValue},
};

/// Number of points the response curve is drawn with.
pub const PLOT_POINTS: usize = 120;

/// How far each band's gain can be pushed, in dB.
pub const EQ_DB_GAIN: f32 = 15.0;

/// The range the graphic EQ pedal's graph and frequency plot are drawn over. The analyser of the
/// pedal covers the same range, so the spectrum and the response curve share a frequency axis.
pub const MIN_FREQ_HZ: f32 = 30.0;
pub const MAX_FREQ_HZ: f32 = 16000.0;

/// The centre frequency of each band, marked with a grey bar down the graph.
///
/// The bands are a little under an octave apart from each other, which is the spacing a graphic EQ
/// is usually set out at, and they take the band at either end of the range to the ends of it.
pub const BAND_FREQS: [f32; 10] = [
    32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

/// The number of bands, and so the length of the gain and bandwidth arrays.
pub const BAND_COUNT: usize = 10;

/// The gain and bandwidth every band starts at.
const INITIAL_GAIN: f32 = 0.0;
const INITIAL_BANDWIDTH: f32 = 1.05;

/// The step a band's gain knob moves in. These match the graphic EQ pedal's, so a band responds to
/// a drag in the same way in both.
const GAIN_STEP: f32 = 0.1;

/// The sample rate the editor draws the response curve for. Only the processor's own rate matters
/// for the audio, and it is given the settings rather than the curve.
const EDITOR_SAMPLE_RATE: f32 = 48000.0;

/// The share of the width it is given the editor draws itself, its bands and its graph across, so
/// that an EQ takes up the working area of the settings screen rather than the whole width of it.
const EDITOR_WIDTH_FRACTION: f32 = 0.6;

/// The height one of the lines a band's labels are drawn in.
const LABEL_HEIGHT: f32 = 20.0;

/// The size a band's name and the gain it is set to are written in, a little under the size the
/// rest of the settings screen is written in, so that the ten of them an editor draws fit the width
/// the bands are spread across without reaching into one another.
const LABEL_FONT_SIZE: f32 = 15.0;

/// The space left between the response graph and the band names above it. A band's knob is drawn
/// inside the graph, so the names are only held off it by enough to keep the graph and the names
/// apart.
const NAMES_PADDING: f32 = 8.0;

/// The space left between the response graph and the gains read out below it, so that a gain reads
/// as being the gain of the band under it rather than as belonging to the graph above it.
const READOUTS_PADDING: f32 = 28.0;

/// The share of the width of its column a band's gain knob is drawn at, so that the response the
/// graph is drawn with can be read either side of it.
const KNOB_WIDTH_FRACTION: f32 = 0.5;

/// The share of the height of a band's slot its gain knob travels over, so that a knob at either
/// end of the range a band is set over is drawn inside the graph rather than reaching up over the
/// band names above it or down over the gain read out below it. What is left of the slot at either
/// end is the room the knob itself takes up there, so the knob and the graph are laid out over the
/// same height.
const KNOB_TRAVEL_FRACTION: f32 = 0.9;

/// A named EQ that can be applied to the whole signal chain.
///
/// Both shelves are always on, like the graphic EQ pedal's, so a band at either end of the range
/// shapes everything past it instead of a narrow peak. Any field a saved EQ is missing is left at
/// its default, and it can be saved holding a different number of bands than the editor has now.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct GraphicEqSettings {
    pub name: String,
    #[serde(deserialize_with = "saved_bands::gains")]
    pub gains: [f32; BAND_COUNT],
    #[serde(deserialize_with = "saved_bands::bandwidths")]
    pub bandwidths: [f32; BAND_COUNT],
}

/// Reading the bands of a saved EQ.
///
/// An EQ is saved with the bands the editor had when it was saved, and the editor has been given a
/// different number of bands than it used to have, so a saved EQ can hold fewer or more bands than
/// the editor has now. Its bands are read into the bands the editor has rather than the whole EQ
/// being left unreadable, which would take the settings it was saved in with it.
mod saved_bands {
    use super::*;
    use serde::Deserializer;

    pub fn gains<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[f32; BAND_COUNT], D::Error> {
        bands(deserializer, INITIAL_GAIN)
    }

    pub fn bandwidths<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[f32; BAND_COUNT], D::Error> {
        bands(deserializer, INITIAL_BANDWIDTH)
    }

    /// Read the bands a saved EQ holds into the bands the editor has, from the low end of the range
    /// up, giving any band the saved EQ holds none of `fill`
    fn bands<'de, D: Deserializer<'de>>(
        deserializer: D,
        fill: f32,
    ) -> Result<[f32; BAND_COUNT], D::Error> {
        let saved = Vec::<f32>::deserialize(deserializer)?;
        let mut bands = [fill; BAND_COUNT];

        for (band, saved) in bands.iter_mut().zip(saved) {
            *band = saved;
        }

        Ok(bands)
    }
}

impl Default for GraphicEqSettings {
    fn default() -> Self {
        Self {
            name: String::new(),
            gains: [INITIAL_GAIN; BAND_COUNT],
            bandwidths: [INITIAL_BANDWIDTH; BAND_COUNT],
        }
    }
}

impl GraphicEqSettings {
    /// An EQ with every band flat
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// The filters this EQ applies at a sample rate. The processor builds them for the rate it
    /// processes audio at, which is higher than the device's when it is upsampling.
    pub fn build_eq(&self, sample_rate: f32) -> Equalizer {
        // Both shelves are always part of the EQ, so the band at either end of the range shapes
        // everything past it rather than leaving a narrow peak at the end of the range
        eq::GraphicEqualizerBuilder::new(sample_rate)
            .with_bands(BAND_FREQS)
            .with_bandwidths(self.bandwidths)
            .with_gains(self.gains)
            .with_upper_shelf()
            .with_lower_shelf()
            .build()
    }

    /// The response curve the editor graphs, sampled over the range the graph is drawn across, from
    /// one end of it to the other
    pub fn response_plot(&self) -> Vec<PlotPoint> {
        let (low, high) = graph_freq_range();

        self.build_eq(EDITOR_SAMPLE_RATE).amplitude_response_plot(
            EDITOR_SAMPLE_RATE as f64,
            low,
            high,
            PLOT_POINTS,
        )
    }
}

/// The EQs a user has saved, and the one that is applied to the signal chain.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct EqPresets {
    pub eqs: Vec<GraphicEqSettings>,
    /// Index into `eqs` of the EQ that is applied, `None` for no EQ at all
    pub selected: Option<usize>,
}

impl EqPresets {
    /// The EQ that is applied to the signal chain, if any
    pub fn selected_eq(&self) -> Option<&GraphicEqSettings> {
        self.selected.and_then(|index| self.eqs.get(index))
    }

    /// The EQ that is applied to the signal chain, if any
    pub fn selected_eq_mut(&mut self) -> Option<&mut GraphicEqSettings> {
        self.selected.and_then(|index| self.eqs.get_mut(index))
    }

    /// Apply the EQ at `index`, or no EQ at all. An index that holds no EQ applies none, so a
    /// selection left over from an older save cannot point at nothing.
    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index.filter(|index| *index < self.eqs.len());
    }

    /// Add an EQ with every band flat, apply it, and return its index
    pub fn add(&mut self, name: impl Into<String>) -> usize {
        self.eqs.push(GraphicEqSettings::new(name));
        let index = self.eqs.len() - 1;
        self.selected = Some(index);
        index
    }

    /// Delete the EQ at `index`. The EQ that takes its place in the list is applied in its stead,
    /// so deleting the applied EQ switches to the next one instead of leaving none applied.
    /// Returns whether an EQ was deleted.
    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.eqs.len() {
            return false;
        }

        self.eqs.remove(index);

        self.selected = match self.selected {
            Some(selected) if selected > index => Some(selected - 1),
            Some(index) => (index < self.eqs.len()).then_some(index),
            selected => selected,
        };

        true
    }

    /// Rename the EQ at `index`, returning whether the name was changed
    pub fn rename(&mut self, index: usize, name: String) -> bool {
        match self.eqs.get_mut(index) {
            Some(eq) => {
                eq.name = name;
                true
            }
            None => false,
        }
    }

    /// A name no EQ is using yet, so a new EQ can always be told apart in the dropdown
    pub fn unused_name(&self) -> String {
        let mut number = self.eqs.len() + 1;

        loop {
            let name = format!("EQ {number}");
            if !self.eqs.iter().any(|eq| eq.name == name) {
                return name;
            }

            number += 1;
        }
    }
}

/// The range of frequencies the response graph is drawn over.
///
/// It reaches half a band's share of the range past the band at either end, so that every band's
/// bar is drawn over the column its name, its gain knob and its gain readout are laid out in rather
/// than a little to one side of it: the bands are set out evenly across the graph, one share of it
/// each, and the graph is drawn half a share past either end of them. The response is sampled over
/// the same range, so the curve reaches both ends of the graph rather than stopping short of the
/// end it was given.
fn graph_freq_range() -> (f64, f64) {
    let lowest = BAND_FREQS[0] as f64;
    let highest = BAND_FREQS[BAND_COUNT - 1] as f64;
    // The bands are a little under an octave apart rather than exactly, so the share of the range
    // one band is given is taken from the range the bands are set out over as a whole
    let half_band = (highest.log2() - lowest.log2()) / (BAND_COUNT - 1) as f64 / 2.0;

    (
        2f64.powf(lowest.log2() - half_band),
        2f64.powf(highest.log2() + half_band),
    )
}

/// The name of a band, in Hz or kHz
fn band_name(band: usize) -> String {
    let freq = BAND_FREQS[band];

    if freq >= 1000.0 {
        format!("{:.1} kHz", freq / 1000.0)
    } else {
        format!("{freq:.0} Hz")
    }
}

/// The parameter a band's gain knob is given. It carries the pedal's range, so a band is dragged
/// through the same range in the editor as on the pedal.
fn gain_parameter(gain: f32) -> PedalParameter {
    PedalParameter {
        value: PedalParameterValue::Float(gain),
        min: Some(PedalParameterValue::Float(-EQ_DB_GAIN)),
        max: Some(PedalParameterValue::Float(EQ_DB_GAIN)),
        step: Some(PedalParameterValue::Float(GAIN_STEP)),
    }
}

/// Editor for an EQ, drawn over the share of the width it is given and in the middle of it: a
/// band's name above the response graph, the graph with the band's gain knob drawn over it, and the
/// gain it is set to below it. A band's gain is the only value the editor writes on itself, in dB:
/// a band's bandwidth is not one of the things it shows.
///
/// A band's knob travels over the middle of the height of the graph, so that a knob at either end
/// of the range a band is set over is drawn inside the graph rather than over the names above it or
/// the gains read out below it. The graph is held off each of those two rows of labels by a padding
/// of its own, so that a band's name and the gain it is set to read as belonging to the band they
/// are drawn beside, and a band's labels are written a little smaller than the rest of the screen
/// is, so that the ten of them an editor draws fit the width the bands are spread across.
///
/// Every size the editor lays a widget out over is worked out here, from the width the editor is
/// given, rather than from the space the widget reports. A ui inside a scroll area reports an
/// endless height, and an image reports no size at all until it has loaded, so a column sized from
/// either of those would come out a different size every frame.
///
/// `id` names the widget the editor's graph is drawn as, so that the graph of one editor is not
/// taken for the graph of another.
///
/// Returns whether the user changed the EQ.
pub fn graphic_eq_editor_ui(ui: &mut egui::Ui, settings: &mut GraphicEqSettings, id: u32) -> bool {
    // A band is given an equal share of the width rather than the width its labels need, so that
    // the bands are spread across the graph instead of crowding into one side of it, and an editor
    // given less width than its labels need is widened to the width they do need rather than
    // drawing each label over the band beside it. A band's column is no wider than that share, its
    // knob is drawn a share of the width of its column, and its slot is drawn the width of its knob
    let available_width = ui.available_width();
    let widest_label = label_width(ui);
    let editor_width = editor_width(available_width, widest_label);
    let band_pitch = editor_width / BAND_COUNT as f32;
    let band_width = widest_label.min(band_pitch);
    let knob_width = band_width * KNOB_WIDTH_FRACTION;

    // The rows the editor is laid out over, one below the last: the bands' names, the graph, and
    // the gains the bands are set to. A band's gain knob is drawn over the graph, travelling the
    // middle of its height, so that a knob at either end of the range a band is set over is drawn
    // inside the graph rather than reaching up over the names above it or down over the readouts
    // below it. The slot the knob travels along is drawn the height of the graph and the width of
    // the knob, so that it is the track the knob is read against rather than a column reaching out
    // either side of it, and the height a band's gain is read at on the graph is the place its knob
    // is drawn at along its slot. The graph is drawn in the proportions of a band's slot: a slot
    // drawn the width of a band's column is this many times as tall as it is wide
    let graph_height = band_width * SLOT_ASPECT_RATIO;
    let height = LABEL_HEIGHT + NAMES_PADDING + graph_height + READOUTS_PADDING + LABEL_HEIGHT;

    // The editor takes that share of the width it was given and that height, and is drawn in the
    // middle of the width it was given, so that an EQ sits in the middle of the settings screen.
    // Everything it draws is drawn inside that, so that no part of it can reach past the space it
    // was given
    let (row_rect, _) = ui.allocate_exact_size(Vec2::new(available_width, height), Sense::hover());
    let editor_rect = row_rect
        .with_min_x(row_rect.center().x - editor_width / 2.0)
        .with_max_x(row_rect.center().x + editor_width / 2.0);

    let (names_rect, rows) = editor_rect.split_top_bottom_at_y(editor_rect.top() + LABEL_HEIGHT);
    let (_, rows) = rows.split_top_bottom_at_y(rows.top() + NAMES_PADDING);
    let (graph_rect, rows) = rows.split_top_bottom_at_y(rows.top() + graph_height);
    let (_, rows) = rows.split_top_bottom_at_y(rows.top() + READOUTS_PADDING);
    let (readouts_rect, _) = rows.split_top_bottom_at_y(rows.top() + LABEL_HEIGHT);

    let mut changed = false;

    // Each band's name is drawn above the graph, centred over the band's own share of the width
    band_labels_ui(ui, names_rect, band_pitch, |band, ui| {
        centered_label(ui, band_name(band))
    });

    // The graph is drawn before the bands, so that it is behind them and the response the bands
    // give can be seen beside them while they are dragged. It is drawn in a ui of its own, as a
    // plot is drawn where the ui it is placed in has its cursor
    response_plot_ui(
        &mut ui.new_child(UiBuilder::new().max_rect(graph_rect)),
        settings,
        id,
        graph_rect.size(),
    );

    // Each band's gain knob is centred over the band's own share of the width, and drawn over the
    // graph rather than beside it. The slot it travels along is given the middle of that share, so
    // that the slot and the knob are drawn the width of the knob where the band's gain is read
    for band in 0..BAND_COUNT {
        changed |= band_gain_ui(
            ui,
            settings,
            band,
            band_rect(graph_rect, band_pitch, band, knob_width),
        );
    }

    // The gains the bands are set to are read out below the graph, under the band's column, so that
    // the graph is left to the response the gain knobs give
    band_labels_ui(ui, readouts_rect, band_pitch, |band, ui| {
        centered_label(ui, gain_readout(settings.gains[band]))
    });

    changed
}

/// The font a band's name and the gain it is set to are written in
fn label_font() -> egui::FontId {
    egui::FontId::proportional(LABEL_FONT_SIZE)
}

/// The width the labels above and below a band's knobs need.
///
/// Every label is centred over the column it belongs to, so a column narrower than one of its
/// labels would have the label reach into the space between two bands, which is what leaves a
/// band's name and the gain it is set to looking squeezed together.
fn label_width(ui: &egui::Ui) -> f32 {
    let font = label_font();

    let widest_name = (0..BAND_COUNT)
        .map(|band| text_width(ui, &band_name(band), &font))
        .fold(0.0_f32, f32::max);
    // A gain readout is at its widest at either end of the range a band can be set over
    let widest_readout = text_width(ui, &gain_readout(-EQ_DB_GAIN), &font);

    widest_name.max(widest_readout)
}

/// The width an editor draws itself, its bands and its graph across, given the width it is given to
/// lay itself out in and the width its labels need.
///
/// It is a share of that width, so that an EQ takes up the working area of the settings screen
/// rather than the whole width of it, but never narrower than the columns its labels need: ten
/// labels written across an editor narrower than that would be drawn over one another. It is never
/// wider than the width it is given either, as ten labels are drawn over one another whatever the
/// editor does with the width once it has run out of it.
fn editor_width(available_width: f32, label_width: f32) -> f32 {
    (available_width * EDITOR_WIDTH_FRACTION)
        .max(label_width * BAND_COUNT as f32)
        .min(available_width)
}

/// The space one band's share of a row takes up: `width` wide and as tall as the row, centred over
/// the band's column, so that everything drawn for a band lines up in a column with the band
fn band_rect(row_rect: egui::Rect, band_pitch: f32, band: usize, width: f32) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(
            row_rect.left() + band_pitch * (band as f32 + 0.5) - width / 2.0,
            row_rect.top(),
        ),
        Vec2::new(width, row_rect.height()),
    )
}

/// The text a band's gain is read out as
fn gain_readout(gain: f32) -> String {
    format!("{gain:.1} dB")
}

/// The width a piece of text is drawn at in a font
fn text_width(ui: &egui::Ui, text: &str, font: &egui::FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_string(), font.clone(), Color32::PLACEHOLDER)
        .size()
        .x
}

/// A band's gain knob: the slot its gain is set along, with the knob drawn at the gain's place
/// along the middle of it. Returns whether the user changed the gain.
fn band_gain_ui(
    ui: &mut egui::Ui,
    settings: &mut GraphicEqSettings,
    band: usize,
    band_slot: egui::Rect,
) -> bool {
    // The slot is given exactly the rectangle it was laid out over, so that nothing drawn in it can
    // make it any wider or any taller: a wider slot would reach over the band beside it, and a
    // taller one would reach past the row the band's knobs are drawn in
    let mut slot = ui.new_child(
        UiBuilder::new()
            .max_rect(band_slot)
            .layout(Layout::top_down(Align::Center)),
    );

    let gain = gain_parameter(settings.gains[band]);

    match gain_knob(
        &mut slot,
        &gain,
        band_slot.size(),
        band_slot.width(),
        KNOB_TRAVEL_FRACTION,
    ) {
        Some(value) => {
            settings.gains[band] = value;
            true
        }
        None => false,
    }
}

/// Draw a label for every band in the row it is given, each centred over the band's own share of
/// the width
fn band_labels_ui(
    ui: &mut egui::Ui,
    row_rect: egui::Rect,
    band_pitch: f32,
    mut label: impl FnMut(usize, &mut egui::Ui),
) {
    for band in 0..BAND_COUNT {
        label(
            band,
            &mut ui.new_child(
                UiBuilder::new()
                    .max_rect(band_rect(row_rect, band_pitch, band, band_pitch))
                    .layout(Layout::top_down(Align::Center)),
            ),
        );
    }
}

/// A label centred over a band's column, in the height a column sets aside for one. Its text is
/// never wrapped, as a band's name can be wider than the column it is centred over when the editor
/// is given little width to lay the bands and the graph out across.
fn centered_label(ui: &mut egui::Ui, text: String) {
    let width = ui.available_width();

    ui.allocate_ui_with_layout(
        Vec2::new(width, LABEL_HEIGHT),
        Layout::top_down(Align::Center),
        |ui| {
            ui.add(
                egui::Label::new(RichText::new(text).font(label_font()))
                    .wrap_mode(egui::TextWrapMode::Extend),
            );
        },
    );
}

/// The response graph of an EQ, with a bar marking each band's centre frequency
fn response_plot_ui(ui: &mut egui::Ui, settings: &GraphicEqSettings, id: u32, size: Vec2) {
    let response_plot = settings.response_plot();
    // A band's gain is applied in full at its centre, and bands overlap, so a curve can reach
    // higher than the range a single band's knob is allowed. The fill is drawn down to whatever
    // the curve needs instead of to the mapped floor, which would leave the shading floating.
    let fill_bottom = frequency_plot::live_plot_fill_bottom(&response_plot, &[], EQ_DB_GAIN as f64);
    // The graph is drawn over the range the bands are set out across, rather than over whatever
    // range the curve happens to cover, so that a band's bar is drawn over the column its labels
    // and its knob are laid out in
    let (low, high) = graph_freq_range();

    Plot::new(("global_eq_response", id))
        .height(size.y)
        .width(size.x)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .show_x(false)
        .show_y(false)
        .center_y_axis(true)
        .show_axes(false)
        .include_x(low.log2())
        .include_x(high.log2())
        .include_y(EQ_DB_GAIN)
        .include_y(-EQ_DB_GAIN)
        .show_grid(false)
        .set_margin_fraction(Vec2::ZERO)
        .show(ui, |plot_ui| {
            plot_ui.line(
                Line::new("freq_response", response_plot.as_slice())
                    .width(1.0_f32)
                    .color(Color32::from_rgb(150, 150, 245))
                    .fill(fill_bottom as f32)
                    .fill_alpha(0.12),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An EQ named after the test it belongs to, with the given bands
    fn eq(gains: [f32; BAND_COUNT], bandwidths: [f32; BAND_COUNT]) -> GraphicEqSettings {
        GraphicEqSettings {
            name: "Test".to_string(),
            gains,
            bandwidths,
        }
    }

    /// The response of an EQ at a frequency, in dB
    fn response_db(settings: &GraphicEqSettings, freq: f32) -> f64 {
        let response = settings
            .build_eq(EDITOR_SAMPLE_RATE)
            .response_at_freq(freq as f64, EDITOR_SAMPLE_RATE as f64);

        20.0 * response.norm().log10()
    }

    /// The bands the editor has, with the given bands at the low end of the range and the bands
    /// beyond them left at `fill`, for checking the bands a saved EQ is read into
    fn bands_with<const N: usize>(bands: [f32; N], fill: f32) -> [f32; BAND_COUNT] {
        let mut all = [fill; BAND_COUNT];
        all[..N].copy_from_slice(&bands);
        all
    }

    /// A set of EQs named after the order they were added in
    fn presets(names: &[&str]) -> EqPresets {
        let mut presets = EqPresets::default();

        for name in names {
            presets.add(name.to_string());
        }

        presets
    }

    #[test]
    fn a_flat_eq_does_not_change_the_response() {
        let flat = GraphicEqSettings::default();

        for freq in [MIN_FREQ_HZ, 100.0, 800.0, 3200.0, MAX_FREQ_HZ] {
            let db = response_db(&flat, freq);
            assert!(db.abs() < 0.01, "{freq} Hz: {db}dB");
        }
    }

    #[test]
    fn a_boosted_band_only_lifts_its_own_frequency() {
        let mut gains = [INITIAL_GAIN; BAND_COUNT];
        gains[3] = EQ_DB_GAIN;
        let settings = eq(gains, [INITIAL_BANDWIDTH; BAND_COUNT]);

        let centre = response_db(&settings, BAND_FREQS[3]);
        assert!(centre > EQ_DB_GAIN as f64 * 0.9, "centre: {centre}dB");

        // The band below is lifted, but by less than the boosted band itself
        let below = response_db(&settings, BAND_FREQS[2]);
        assert!(below < centre, "below: {below}dB");

        // The outermost bands are shelves, but they are left flat, so the ends of the range are
        // left alone
        assert!(response_db(&settings, MIN_FREQ_HZ).abs() < 1.0);
        assert!(response_db(&settings, MAX_FREQ_HZ).abs() < 1.0);
    }

    #[test]
    fn a_band_at_either_end_of_the_range_shapes_everything_past_it() {
        let mut gains = [INITIAL_GAIN; BAND_COUNT];
        gains[0] = EQ_DB_GAIN;
        gains[BAND_COUNT - 1] = EQ_DB_GAIN;
        let boosted = eq(gains, [INITIAL_BANDWIDTH; BAND_COUNT]);

        // The bands at either end of the range are shelves, so they apply the whole of their gain
        // past the ends of the range rather than rolling off like a peak. A shelf takes an octave or
        // so to reach the whole of its gain, so it is read past the ends of the range the graph
        // covers: at the bottom of the frequencies that can be heard, and a little above the top of
        // the range, which is the top of them.
        let low = response_db(&boosted, MIN_FREQ_HZ / 2.0);
        let high = response_db(&boosted, 20_000.0);
        assert!(low > EQ_DB_GAIN as f64 * 0.9, "low end: {low}dB");
        assert!(high > EQ_DB_GAIN as f64 * 0.9, "high end: {high}dB");

        // The same holds for a cut, which a shelf takes further down
        let mut cut_gains = [INITIAL_GAIN; BAND_COUNT];
        cut_gains[0] = -EQ_DB_GAIN;
        cut_gains[BAND_COUNT - 1] = -EQ_DB_GAIN;
        let cut = eq(cut_gains, [INITIAL_BANDWIDTH; BAND_COUNT]);

        let low = response_db(&cut, MIN_FREQ_HZ / 2.0);
        let high = response_db(&cut, 20_000.0);
        assert!(low < -EQ_DB_GAIN as f64 * 0.9, "low end: {low}dB");
        assert!(high < -EQ_DB_GAIN as f64 * 0.9, "high end: {high}dB");
    }

    #[test]
    fn an_eq_saved_by_an_older_editor_still_loads() {
        // An EQ saved before a field existed is loaded with the default for it
        let settings: GraphicEqSettings =
            serde_json::from_str(r#"{"name": "Old"}"#).expect("Failed to load EQ");

        assert_eq!(settings.name, "Old");
        assert_eq!(settings.gains, [INITIAL_GAIN; BAND_COUNT]);
        assert_eq!(settings.bandwidths, [INITIAL_BANDWIDTH; BAND_COUNT]);

        // The shelves are always on now, so an EQ still carrying the fields they used to be turned
        // off with is loaded with the fields ignored. The EQ holds as many bands as the editor had
        // when it was saved, so the bands it holds are read into the bands at the low end of the
        // range.
        let settings: GraphicEqSettings = serde_json::from_str(
            r#"{"name": "Old", "gains": [1,2,3,4,5,6,7], "low_shelf": false, "high_shelf": false}"#,
        )
        .expect("Failed to load EQ");

        assert_eq!(
            settings.gains,
            bands_with([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0], INITIAL_GAIN)
        );

        // An EQ saved with fewer bands than the editor has now is read into the bands at the low
        // end of the range, and one saved with more than it has now is read into as many bands as
        // the editor has, keeping the bands at the low end of the range
        let settings: GraphicEqSettings =
            serde_json::from_str(r#"{"name": "Old", "gains": [1,2,3], "bandwidths": [1.2]}"#)
                .expect("Failed to load EQ");

        assert_eq!(settings.gains, bands_with([1.0, 2.0, 3.0], INITIAL_GAIN));
        assert_eq!(settings.bandwidths, bands_with([1.2], INITIAL_BANDWIDTH));

        let settings: GraphicEqSettings = serde_json::from_str(
            r#"{"name": "Newer", "gains": [1,2,3,4,5,6,7,8,9,10,11,12], "bandwidths": [1,1,1,1,1,1,1,1,1,1,1,1]}"#,
        )
        .expect("Failed to load EQ");

        assert_eq!(
            settings.gains,
            [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
        );
        assert_eq!(settings.bandwidths, [1.0; BAND_COUNT]);

        // An EQ saved holding no bands at all is left at the defaults rather than failing to load,
        // as a settings file that cannot be read would take the rest of the EQs with it
        let settings: GraphicEqSettings =
            serde_json::from_str(r#"{"name": "Empty", "gains": []}"#).expect("Failed to load EQ");

        assert_eq!(settings.gains, [INITIAL_GAIN; BAND_COUNT]);
        assert_eq!(settings.bandwidths, [INITIAL_BANDWIDTH; BAND_COUNT]);

        // A selection left over from a saved file that holds no EQ, or names one that is not in it,
        // applies nothing rather than a setting that is not there
        let eqs: EqPresets =
            serde_json::from_str(r#"{"eqs": [], "selected": 3}"#).expect("Failed to load EQs");

        assert!(eqs.selected_eq().is_none());
    }

    #[test]
    fn the_eq_that_is_applied_is_always_one_of_the_eqs_that_are_kept() {
        // A new EQ is given a name no other EQ is using, and is applied as soon as it is created
        let mut eqs = EqPresets::default();

        let first = eqs.add(eqs.unused_name());
        assert_eq!(eqs.selected, Some(first));
        assert_eq!(eqs.selected_eq().unwrap().name, "EQ 1");

        // Renaming the first EQ to the name a second one would have been given keeps the two
        // apart, as the dropdown is the only place an EQ can be told apart
        assert!(eqs.rename(first, "EQ 2".to_string()));

        let second = eqs.add(eqs.unused_name());
        assert_eq!(eqs.selected, Some(second));
        assert_eq!(eqs.selected_eq().unwrap().name, "EQ 3");

        // Deleting the applied EQ applies the one that takes its place, and deleting one before
        // the applied EQ leaves it applied
        let mut eqs = presets(&["A", "B", "C"]);
        eqs.select(Some(1));

        assert!(eqs.remove(1));
        assert_eq!(eqs.selected_eq().unwrap().name, "C");

        assert!(eqs.remove(0));
        assert_eq!(eqs.selected, Some(0));
        assert_eq!(eqs.selected_eq().unwrap().name, "C");

        // Deleting the last EQ leaves none applied
        assert!(eqs.remove(0));
        assert_eq!(eqs.selected, None);
        assert!(eqs.selected_eq().is_none());
        assert!(eqs.selected_eq_mut().is_none());

        // Deleting an EQ that is not there, or renaming one that is not there, leaves the EQs and
        // the one that is applied alone
        let mut eqs = presets(&["A"]);

        assert!(!eqs.remove(1));
        assert!(!eqs.rename(1, "Renamed".to_string()));
        assert_eq!(eqs.eqs[0].name, "A");
        assert_eq!(eqs.selected_eq().unwrap().name, "A");

        // A selection that is not one of the EQs applies nothing, whether it was left over from a
        // saved file or made by no EQ being chosen
        eqs.select(Some(7));
        assert_eq!(eqs.selected, None);
        assert!(eqs.selected_eq().is_none());

        eqs.select(None);
        assert!(eqs.selected_eq().is_none());
    }
}
