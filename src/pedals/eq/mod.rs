use eframe::egui::{include_image, Image, UiBuilder, Vec2};
use egui_plot::PlotPoint;

use crate::pedals::PedalParameter;

pub mod graphiceq7;
mod simple;
pub use graphiceq7::GraphicEq7;
pub use simple::SimpleEq;

pub fn eq_background(ui: &mut eframe::egui::Ui) {
    let rect = ui.available_rect_before_wrap();
    ui.new_child(UiBuilder::new().max_rect(rect))
        .add(Image::new(include_image!("../images/eq.png")));
}

pub fn gain_knob(ui: &mut eframe::egui::Ui, parameter: &PedalParameter, width: f32) -> Option<f32> {
    ui.vertical(|ui| {
        let slot = Image::new(include_image!("../images/eq/slot.png")).max_width(width);
        let slot_response = ui.add(slot);
        let knob = Image::new(include_image!("../images/eq/knob.png"))
            .max_width(width)
            .sense(eframe::egui::Sense::click_and_drag());
        let knob_size = knob.calc_size(slot_response.rect.size(), None);
        let max = parameter.max.as_ref().unwrap().as_float().unwrap();
        let min = parameter.min.as_ref().unwrap().as_float().unwrap();
        let value = parameter.value.as_float().unwrap();
        let fraction = 1.0 - (value - min) / (max - min);
        let knob_rect = slot_response.rect.translate(Vec2::new(
            0.0,
            slot_response.rect.height() * fraction - knob_size.y / 2.0,
        ));
        let response = ui
            .new_child(
                UiBuilder::new()
                    .max_rect(knob_rect)
                    .layout(eframe::egui::Layout::top_down(eframe::egui::Align::Center))
                    .sense(eframe::egui::Sense::click_and_drag()),
            )
            .add(knob);
        if response.hovered() {
            ui.ctx().output_mut(|output| {
                output.cursor_icon = eframe::egui::CursorIcon::ResizeVertical
            });
        }
        response.dragged().then(|| {
            (value + (-response.drag_delta().y / slot_response.rect.height()) * (max - min))
                .clamp(min, max)
        })
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
