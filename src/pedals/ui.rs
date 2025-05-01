use eframe::egui::{self, include_image, Color32, Image, ImageSource, RichText, Ui};
use eframe::egui::Vec2;

use super::{PedalParameter, PedalParameterValue};

// -120 deg
const KNOB_MIN_ANGLE: f32 = -2.094;
// 120 deg
const KNOB_MAX_ANGLE: f32 = 2.094;

/// Fills the UI with an image, scaling it to fit the available width
#[deprecated]
pub fn fill_ui_with_image_width(ui: &mut Ui, source: ImageSource) {
    let pedal_im = eframe::egui::Image::new(source);
    // let im_size = pedal_im.load_for_size(ui.ctx(), Vec2::new(f32::INFINITY, f32::INFINITY)).unwrap().size().unwrap();
    // let scaled_im_size = im_size * (ui.available_width() / im_size.x);
    //ui.set_max_height(scaled_im_size.y);
    ui.add(pedal_im);
}

pub fn pedal_knob(ui: &mut egui::Ui, name: &str, parameter: &PedalParameter, at: egui::Vec2, size: f32) -> Option<PedalParameterValue> {
    let pedal_parameter_float;

    match parameter.value {
        PedalParameterValue::Float(_) => {
            pedal_parameter_float = parameter.clone();
        },
        PedalParameterValue::Int(_) => {
            pedal_parameter_float = parameter.int_to_float();
        },
        _ => {
            ui.label("Invalid parameter type.");
            return None;
        }
    };

    let value = pedal_parameter_float.value.as_float().unwrap();
    let min = pedal_parameter_float.min.unwrap().as_float().unwrap();
    let max = pedal_parameter_float.max.unwrap().as_float().unwrap();
    let value_fract_between_min_max = (value - min) / (max - min);
    let knob_rotate = KNOB_MIN_ANGLE + value_fract_between_min_max * (KNOB_MAX_ANGLE - KNOB_MIN_ANGLE);
    let mut new_value_float = value;

    let parent_rect = ui.max_rect();
    let size_px = size * parent_rect.width();
    let draw_rect = egui::Rect::from_min_size(
        parent_rect.min + Vec2::new(at.x*parent_rect.width(), at.y*parent_rect.height()),
        Vec2::new(size_px, size_px+8.0)
    );

    ui.allocate_new_ui(
    egui::UiBuilder::new()
        .max_rect(draw_rect)
        .layout(egui::Layout::top_down(egui::Align::Center))
        .sense(egui::Sense::click_and_drag()),
    |ui| {
            let knob_im = ui.add(egui::Image::new(egui::include_image!("images/pedal_knob.png"))
                .rotate(knob_rotate, Vec2::splat(0.5))
                .max_width(size_px)
                .sense(egui::Sense::click_and_drag())
            );

            if knob_im.dragged() {
                let delta = -knob_im.drag_motion().y;
                let scaled = delta * 0.05;
                new_value_float = (value + scaled).clamp(min, max);
            }

            if knob_im.hovered() {
                ui.ctx().output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeVertical);
            }

            ui.label(RichText::new(name).size(8.0).color(Color32::BLACK));
        },
    );

    if new_value_float != value {
        if let Some(old_value_int) = parameter.value.as_int() {
            let new_value_int;
            if new_value_float > value {
                new_value_int = old_value_int + 1;
            } else {
                new_value_int = old_value_int - 1;
            }

            Some(PedalParameterValue::Int(new_value_int))
        } else {
            // TODO: Clamp to step
            Some(PedalParameterValue::Float(new_value_float))
        }
    } else {
        None
    }
}
