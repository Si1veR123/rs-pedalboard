use eframe::egui::{self, Color32, Vec2};

pub fn set_large_checkbox_style(ui: &mut egui::Ui) {
    ui.style_mut().spacing.icon_width = 35.0;
    ui.style_mut().spacing.icon_width_inner = 12.0;
    ui.style_mut().visuals.widgets.inactive.fg_stroke =
        egui::Stroke::new(2.0_f32, Color32::from_rgb(200, 200, 200));
}

pub fn integer_selector(
    ui: &mut egui::Ui,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
) -> bool {
    let slider_width = ui.available_width() * 0.5;
    let button_size = Vec2::new(30.0, 30.0);
    let item_spacing = ui.spacing().item_spacing.x;
    let row_width = button_size.x * 2.0 + slider_width + item_spacing * 2.0;

    let mut changed = false;

    ui.style_mut().spacing.slider_width = slider_width;
    ui.allocate_ui_with_layout(
        Vec2::new(row_width, 30.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            if ui.add_sized(button_size, egui::Button::new("-")).clicked() {
                *value = value.saturating_sub(1).max(*range.start());
                changed = true;
            }

            if ui
                .add_sized(
                    Vec2::new(slider_width, 30.0),
                    egui::Slider::new(value, range.clone()).show_value(false),
                )
                .changed()
            {
                changed = true;
            }

            if ui.add_sized(button_size, egui::Button::new("+")).clicked() {
                *value = value.saturating_add(1).min(*range.end());
                changed = true;
            }
        },
    );
    changed
}

pub fn start_stop_icon(ui: &mut egui::Ui, start: bool, rect: egui::Rect, size: f32) {
    let icon_size = Vec2::splat(size);
    let right_arrow_rect = egui::Align2::CENTER_CENTER.align_size_within_rect(icon_size, rect);
    let points = if start {
        vec![
            right_arrow_rect.left_top(),
            right_arrow_rect.right_center(),
            right_arrow_rect.left_bottom(),
        ]
    } else {
        vec![
            right_arrow_rect.left_top(),
            right_arrow_rect.right_top(),
            right_arrow_rect.right_bottom(),
            right_arrow_rect.left_bottom(),
        ]
    };
    ui.painter().add(egui::Shape::convex_polygon(
        points,
        Color32::from_gray(200),
        egui::Stroke::NONE,
    ));
}
