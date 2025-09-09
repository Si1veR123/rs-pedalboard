use eframe::egui::{self, Color32, Id, Vec2, WidgetText};

use crate::dsp_algorithms::oscillator::{self, Oscillator};

use super::{PedalParameter, PedalParameterValue};

// -150 deg
const KNOB_MIN_ANGLE: f32 = -2.618;
// 150 deg
const KNOB_MAX_ANGLE: f32 = 2.618;

pub fn float_round(value: f32, step: f32) -> f32 {
    let rounded = (value / step).round() * step;
    rounded
}

pub fn pedal_knob(
    ui: &mut egui::Ui,
    name: impl Into<WidgetText>,
    parameter: &PedalParameter,
    at: egui::Vec2,
    size: f32
) -> Option<PedalParameterValue> {
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

    ui.scope_builder(
    egui::UiBuilder::new()
        .max_rect(draw_rect)
        .layout(egui::Layout::top_down(egui::Align::Center))
        .sense(egui::Sense::click_and_drag()),
    |ui| {
            let mut main_knob_im_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(ui.available_rect_before_wrap())
                    .layout(egui::Layout::top_down(egui::Align::Center))
            );

            main_knob_im_ui.add(egui::Image::new(egui::include_image!("images/pedal_knob_blender_base.png"))
                .rotate(knob_rotate, Vec2::splat(0.5))
                .max_width(size_px)
            );

            let knob_im_shine_overlay = ui.add(egui::Image::new(egui::include_image!("images/pedal_knob_blender_shine.png"))
                .max_width(size_px)
                .sense(egui::Sense::click_and_drag())
            );

            
            if knob_im_shine_overlay.dragged() {
                let current_y = ui.input(|i| i.pointer.interact_pos().expect("Failed to get cursor location")).y;

                let (init_y, init_value) = if knob_im_shine_overlay.drag_started() {
                    // Store the initial y position and value of the drag
                    ui.ctx().memory_mut(|m| {
                        m.data.insert_temp(Id::new("knob_drag_init_y"), (current_y, value));
                        (current_y, value)
                    })
                } else {
                    ui.ctx().memory(|m| m.data.get_temp::<(f32, f32)>(Id::new("knob_drag_init_y")).unwrap_or((0.0, 0.0)).clone())
                };

                // Convert delta y to a change in value
                let delta = init_y - current_y;
                let scaled_delta = delta * 0.008; // Sensitivity factor
                let scaled = scaled_delta * (max - min);
                new_value_float = (init_value + scaled).clamp(min, max);

                if let Some(step) = &parameter.step {
                    new_value_float = float_round(new_value_float, step.as_float().unwrap());
                }
            }

            if knob_im_shine_overlay.hovered() {
                ui.ctx().output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeVertical);
            }

            ui.label(name);
        },
    );

    if new_value_float != value {
        if matches!(parameter.value, PedalParameterValue::Int(_)) {
            if new_value_float as i16 != value as i16 {
                Some(PedalParameterValue::Int(new_value_float as i16))
            } else {
                None
            }
        } else {
            Some(PedalParameterValue::Float(new_value_float))
        }
    } else {
        None
    }
}

pub fn pedal_switch(
    ui: &mut egui::Ui,
    active: bool,
    at: egui::Vec2,
    height: f32
) -> Option<bool> {
    let switch_ratio = 1.5;
    let height_px = height * ui.max_rect().height();
    let size = Vec2::new(height_px*switch_ratio, height_px);
    let switch_rect = egui::Rect::from_min_size(
        ui.max_rect().min + Vec2::new(at.x * ui.max_rect().width(), at.y * ui.max_rect().height()),
        size
    );

    let switch_image = if active {
        egui::include_image!("images/switch_pressed_edited.png")
    } else {
        egui::include_image!("images/switch_edited.png")
    };

    let switch_response = ui.allocate_rect(switch_rect, egui::Sense::click());
    egui::Image::new(switch_image)
        .paint_at(ui, switch_rect);

    if switch_response.clicked() {
        Some(!active)
    } else {
        None
    }
}

pub fn sideways_arrow(
    ui: &mut egui::Ui,
    button_rect: egui::Rect,
    left: bool
) {
    let icon_size = Vec2::splat(ui.spacing().icon_width);
    let right_arrow_rect = egui::Align2::CENTER_CENTER.align_size_within_rect(icon_size, button_rect);
    let points = if left {
        vec![right_arrow_rect.right_top(), right_arrow_rect.right_bottom(), right_arrow_rect.left_center()]
    } else {
        vec![right_arrow_rect.left_top(), right_arrow_rect.right_center(), right_arrow_rect.left_bottom()]
    };
    ui.painter().add(
        egui::Shape::convex_polygon(
            points,
            Color32::from_gray(200),
            egui::Stroke::NONE
        )
    );
}

pub fn oscillator_selection_window(
    ui: &mut egui::Ui,
    parameter: &PedalParameter,
    width: f32,
    oscillator_type_only: bool
) -> egui::InnerResponse<Option<Oscillator>> {
    let selected_oscillator = parameter.value.as_oscillator().unwrap();
    let mut new_oscillator = None;

    let response = ui.vertical(|ui| {
        ui.set_max_width(width);

        ui.columns_const(|[sine_ui, square_ui, sawtooth_ui, triangle_ui]| {
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
            ui.label("Frequency (Hz)");
            let mut frequency_value = selected_oscillator.get_frequency();

            let frequency_range = {
                let min_freq = parameter.min.as_ref().and_then(|p| p.as_float()).unwrap_or(0.0);
                let max_freq = parameter.max.as_ref().and_then(|p| p.as_float()).unwrap_or(20.0);
                min_freq..=max_freq
            };
            ui.add(egui::Slider::new(&mut frequency_value, frequency_range).logarithmic(true).max_decimals(2));

            if frequency_value != selected_oscillator.get_frequency() {
                let mut cloned = selected_oscillator.clone();
                cloned.set_frequency(frequency_value);
                new_oscillator = Some(cloned);
            }

            // Phase
            ui.label("Phase Offset");
            let mut phase_offset_value = selected_oscillator.get_phase_offset();
            ui.add(egui::Slider::new(&mut phase_offset_value, -0.5..=0.5).max_decimals(2));

            if phase_offset_value != selected_oscillator.get_phase_offset() {
                let mut cloned = selected_oscillator.clone();
                cloned.set_phase_offset(phase_offset_value);
                new_oscillator = Some(cloned);
            }

            // Sine-specific squareness
            if let Oscillator::Sine(sine) = selected_oscillator {
                ui.label("Squareness");
                let mut squareness_value = sine.get_squareness();
                ui.add(egui::Slider::new(&mut squareness_value, 0.0..=1.0).max_decimals(2));
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
    }).response;

    egui::InnerResponse::new(new_oscillator, response)
}
