use eframe::egui::{Color32, Rect, Response, Sense, Ui, Vec2, Widget};
use rs_pedalboard::DEFAULT_VOLUME_MONITOR_UPDATE_RATE;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct VolumeMonitorWidget {
    current_volume: f32,
    target_volume: f32,
    volume_limit: Option<f32>,
    smoothing_factor: f32,
    bar_color: Color32,
    clipping: (bool, Instant),
}

impl VolumeMonitorWidget {
    pub fn new(bar_color: Color32) -> Self {
        let smoothing_factor = Self::compute_smoothing_factor(DEFAULT_VOLUME_MONITOR_UPDATE_RATE);

        Self {
            current_volume: 0.0,
            target_volume: 0.0,
            volume_limit: None,
            smoothing_factor,
            bar_color,
            clipping: (false, Instant::now()),
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.target_volume = volume.clamp(0.0, 1.0);

        if volume >= 1.0 {
            self.clipping.0 = true;
            self.clipping.1 = Instant::now();
        } else if self.clipping.0
            && Instant::now().duration_since(self.clipping.1) > super::CLIPPING_STATE_DURATION
        {
            self.clipping.0 = false; // Reset clipping state after 2 seconds of no clipping
        }
    }

    pub fn set_volume_limit(&mut self, limit: f32) {
        self.volume_limit = Some(limit.clamp(0.0, 1.0));
    }

    fn compute_smoothing_factor(update_interval: Duration) -> f32 {
        let tau = 0.3;
        let dt = update_interval.as_secs_f32();
        (1.0 - (-dt / tau).exp()).clamp(0.0, 1.0)
    }

    fn apply_smoothing(&mut self) {
        self.current_volume += self.smoothing_factor * (self.target_volume - self.current_volume);
    }

    fn draw_bar(ui: &mut Ui, rect: Rect, volume: f32, bar_color: Color32, border: Option<f32>) {
        let volume_height = rect.height() * volume;
        let bar_rect = Rect::from_min_max(
            rect.left_bottom() - Vec2::Y * volume_height,
            rect.right_bottom(),
        );

        ui.painter().rect_filled(bar_rect, 1.0, bar_color);

        if let Some(border_width) = border {
            ui.painter().rect_stroke(
                bar_rect,
                1.0,
                (border_width, Color32::RED),
                eframe::egui::StrokeKind::Outside,
            );
        }
    }
}

impl Widget for &mut VolumeMonitorWidget {
    fn ui(self, ui: &mut Ui) -> Response {
        self.apply_smoothing();

        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::hover());

        let vertical_padding = 0.01 * rect.height();

        let padded_rect = rect.shrink2(Vec2::new(0.0, vertical_padding));


        if let Some(limit) = self.volume_limit {
            // Dont draw out of bounds or normal (1) volume limit
            if limit > 0.0 && limit < 1.0 {
                // Draw a gray bar behind the volume bar to indicate the limit
                VolumeMonitorWidget::draw_bar(ui, padded_rect, limit, Color32::from_gray(100), None);
            }
        }

        let (color, border) = if self.clipping.0 {
            (Color32::DARK_RED, Some(3.0))
        } else {
            (self.bar_color, None)
        };
        VolumeMonitorWidget::draw_bar(ui, padded_rect, self.current_volume, color, border);

        response
    }
}
