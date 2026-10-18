use eframe::egui::{Color32, Rect, Response, Sense, Ui, Vec2, Widget};
use rs_pedalboard::{DEFAULT_VOLUME_MONITOR_UPDATE_RATE, dsp::MINIMUM_DB};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct VolumeMonitorWidget {
    current_volume_db: f32,
    target_volume_db: f32,
    volume_limit_db: Option<f32>,
    smoothing_factor: f32,
    bar_color: Color32,
    clipping: (bool, Instant),
}

impl VolumeMonitorWidget {
    pub fn new(bar_color: Color32) -> Self {
        let smoothing_factor = Self::compute_smoothing_factor(DEFAULT_VOLUME_MONITOR_UPDATE_RATE);

        Self {
            current_volume_db: 0.0,
            target_volume_db: 0.0,
            volume_limit_db: None,
            smoothing_factor,
            bar_color,
            clipping: (false, Instant::now()),
        }
    }

    pub fn set_volume_db(&mut self, volume_db: f32) {
        self.target_volume_db = volume_db.clamp(MINIMUM_DB, 0.0);

        if volume_db >= 0.0 {
            self.clipping.0 = true;
            self.clipping.1 = Instant::now();
        } else if self.clipping.0
            && Instant::now().duration_since(self.clipping.1) > super::CLIPPING_STATE_DURATION
        {
            self.clipping.0 = false; // Reset clipping state after 2 seconds of no clipping
        }
    }

    pub fn set_volume_limit_db(&mut self, limit_db: f32) {
        self.volume_limit_db = Some(limit_db.clamp(MINIMUM_DB, 0.0));
    }

    fn compute_smoothing_factor(update_interval: Duration) -> f32 {
        let tau = 0.3;
        let dt = update_interval.as_secs_f32();
        (1.0 - (-dt / tau).exp()).clamp(0.0, 1.0)
    }

    fn apply_smoothing(&mut self) {
        self.current_volume_db += self.smoothing_factor * (self.target_volume_db - self.current_volume_db);
    }

    fn draw_bar(ui: &mut Ui, rect: Rect, volume_fract: f32, bar_color: Color32, border: Option<f32>) {
        let volume_height = rect.height() * volume_fract;
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



        if let Some(limit_db) = self.volume_limit_db {
            // Dont draw out of bounds or normal volume limit
            if limit_db < 0.0 && limit_db > MINIMUM_DB {
                // Draw a gray bar behind the volume bar to indicate the limit
                let limit_fract = (limit_db - MINIMUM_DB) / (0.0 - MINIMUM_DB);
                VolumeMonitorWidget::draw_bar(ui, padded_rect, limit_fract, Color32::from_gray(100), None);
            }
        }

        let (color, border) = if self.clipping.0 {
            (Color32::DARK_RED, Some(3.0))
        } else {
            (self.bar_color, None)
        };
        let volume_fract = (self.current_volume_db - MINIMUM_DB) / (0.0 - MINIMUM_DB);
        VolumeMonitorWidget::draw_bar(ui, padded_rect, volume_fract, color, border);

        response
    }
}
