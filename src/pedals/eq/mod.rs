use eframe::egui::{include_image, Align, Image, Layout, Sense, UiBuilder, Vec2};
use egui_plot::PlotPoint;

use crate::pedals::PedalParameter;

pub mod frequency_plot;
pub mod global_eq;
pub mod graphiceq7;
mod simple;
pub use global_eq::{
    graphic_eq_editor_ui, EqPresets, GraphicEqSettings, BAND_COUNT, BAND_FREQS, EQ_DB_GAIN,
    MAX_FREQ_HZ, MIN_FREQ_HZ, PLOT_POINTS,
};
pub use graphiceq7::GraphicEq7;
pub use simple::SimpleEq;

/// `slot.png` is 39x298, so a band's gain slot is this many times as tall as it is wide.
pub const SLOT_ASPECT_RATIO: f32 = 298.0 / 39.0;

/// `knob.png` is 160x255, so a band's gain knob is this many times as tall as it is wide.
pub const KNOB_ASPECT_RATIO: f32 = 255.0 / 160.0;

pub fn eq_background(ui: &mut eframe::egui::Ui) {
    let rect = ui.available_rect_before_wrap();
    ui.new_child(UiBuilder::new().max_rect(rect))
        .add(Image::new(include_image!("../images/eq.png")));
}

/// A band's gain knob: the slot that marks the range a band's gain is set over, with the knob
/// drawn at the gain's place along it. Returns the gain the user has dragged the knob to.
///
/// The slot is drawn at the size it is given, which a caller works out from the space a band is
/// laid out over, and the knob at the width it is given, in the proportions its own image works
/// out to. A caller that draws a slot at the size a band's column works out to draws a knob as wide
/// as the slot, and one that draws a slot for a knob to be read against draws a narrower knob on
/// it, which leaves the slot showing either side of the knob.
///
/// The knob travels over `travel_fraction` of the height of the slot, centred on it, so that the
/// knob takes up a share of the slot rather than reaching past either end of it: a caller that
/// draws a slot at the size of the range a band is set over gives `1.0`, and one that draws a
/// taller slot for a knob to travel along the middle of gives the share of it the knob is set over.
///
/// Both images are drawn at the size they are given, rather than at the size they report or at
/// whatever size fits the space they are placed in. An image reports no size at all until it has
/// loaded, and the space a knob is given shrinks as the knobs before it are placed, so either would
/// make a knob a different size in every column and every frame.
pub fn gain_knob(
    ui: &mut eframe::egui::Ui,
    parameter: &PedalParameter,
    slot_size: Vec2,
    knob_width: f32,
    travel_fraction: f32,
) -> Option<f32> {
    ui.vertical(|ui| {
        let knob_size = Vec2::new(knob_width, knob_width * KNOB_ASPECT_RATIO);

        let slot_response = ui.add(
            Image::new(include_image!("../images/eq/slot.png"))
                .fit_to_exact_size(slot_size)
                // The slot is drawn at the size it is given rather than one fitted to whatever
                // size the image reports
                .maintain_aspect_ratio(false),
        );
        let max = parameter.max.as_ref().unwrap().as_float().unwrap();
        let min = parameter.min.as_ref().unwrap().as_float().unwrap();
        let value = parameter.value.as_float().unwrap();
        let fraction = 1.0 - (value - min) / (max - min);
        // The knob is centred over the slot, at the gain's place along the height it travels, which
        // is the middle of the slot rather than the whole of it
        let travel = slot_response.rect.height() * travel_fraction;
        let travel_top = slot_response.rect.center().y - travel / 2.0;
        let knob_rect = eframe::egui::Rect::from_center_size(
            eframe::egui::pos2(
                slot_response.rect.center().x,
                travel_top + travel * fraction,
            ),
            knob_size,
        );
        // The knob is the widget that senses the drag rather than the ui it is drawn in: a ui's
        // sense is reported against the ui itself and is not passed on to the widgets drawn in it,
        // so a drag taken by the ui would leave the knob reading the response of a widget that
        // never senses a drag, and the knob could not be moved
        let response = ui
            .new_child(
                UiBuilder::new()
                    .max_rect(knob_rect)
                    .layout(Layout::top_down(Align::Center))
                    .sense(Sense::click_and_drag()),
            )
            .add(
                Image::new(include_image!("../images/eq/knob.png"))
                    .fit_to_exact_size(knob_size)
                    .maintain_aspect_ratio(false)
                    .sense(Sense::click_and_drag()),
            );
        if response.hovered() {
            ui.ctx()
                .output_mut(|output| output.cursor_icon = eframe::egui::CursorIcon::ResizeVertical);
        }
        response
            .dragged()
            .then(|| (value + (-response.drag_delta().y / travel) * (max - min)).clamp(min, max))
    })
    .inner
}

pub fn serialize_plot_points(plot_points: &mut [PlotPoint]) -> String {
    for point in plot_points.iter_mut() {
        point.x = (point.x * 100.0).round() / 100.0;
        point.y = (point.y * 100.0).round() / 100.0;
    }

    let plot_points_floats: Vec<[f64; 2]> = plot_points.iter().map(|p| [p.x, p.y]).collect();
    serde_json::to_string(&plot_points_floats).expect("Failed to serialize plot points")
}

pub fn deserialize_plot_points(data: &str) -> serde_json::Result<Vec<PlotPoint>> {
    let plot_points_floats: Vec<[f64; 2]> = serde_json::from_str(data)?;
    Ok(plot_points_floats
        .into_iter()
        .map(|p| PlotPoint::new(p[0], p[1]))
        .collect())
}
