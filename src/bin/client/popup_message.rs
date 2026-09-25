use eframe::egui;

const POPUP_MESSAGE_INFO_TIME: f32 = 2.5;
const POPUP_MESSAGE_WARNING_TIME: f32 = 5.0;
const POPUP_MESSAGE_ERROR_TIME: f32 = 7.5;
const POPUP_MESSAGE_ANIMATION_TIME: f32 = 0.25;

// Appearance of the messages, which are displayed at the top right of the window
const MESSAGE_EDGE_MARGIN: f32 = 12.0;
const MESSAGE_PADDING: egui::Vec2 = egui::Vec2::new(16.0, 8.0);
const MESSAGE_CORNER_RADIUS: f32 = 5.0;
const MESSAGE_BACKGROUND_COLOR: egui::Color32 = egui::Color32::from_gray(60);
const MESSAGE_TINT_STRENGTH: f32 = 0.4;
const MESSAGE_OPACITY: f32 = 0.8;
const MESSAGE_TEXT_COLOR: egui::Color32 = crate::TEXT_COLOR;
const ANIMATION_BOUNCE: f32 = 0.25;


macro_rules! popup {
    ($ctx:expr, $message:expr, $message_type:expr, $time_left:expr, $merge_with:expr) => {
        $crate::popup_message::push_message(
            $ctx,
            $message_type,
            $message,
            $time_left,
            $merge_with,
        );
    };
    ($ctx:expr, $message:expr, $merge_with:expr, $message_type:expr) => {
        $crate::popup_message::push_message($ctx, $message_type, $message, None, $merge_with);
    };
    ($ctx:expr, $message:expr, $merge_with:expr) => {
        $crate::popup_message::push_message($ctx, $crate::popup_message::PopupMessageType::Info, $message, None, $merge_with);
    };
    ($ctx:expr, $message:expr) => {
        $crate::popup_message::push_message(
            $ctx,
            $crate::popup_message::PopupMessageType::Info,
            $message,
            None,
            None,
        );
    };
}
pub(crate) use popup;

// Easing functions from https://github.com/michaelfairley/ezing/tree/master
#[inline]
pub fn back_in(t: f32) -> f32 {
  t * t * t - ANIMATION_BOUNCE * t * (t * std::f32::consts::PI).sin()
}

#[inline]
pub fn back_out(t: f32) -> f32 {
  let f = 1.0 - t;
  1.0 - f * f * f + ANIMATION_BOUNCE * f * (f * std::f32::consts::PI).sin()
}

#[derive(Clone)]
pub enum PopupMessageType {
    Info,
    Warning,
    Error
}

#[derive(Clone)]
struct PopupMessage {
    message_type: PopupMessageType,
    message: String,
    time_left: f32,
    merge_with: Option<String>,
    timestamp: std::time::Instant,
}

/// Queues a message which the `PopupWidget` picks up on the next frame. Called by the
/// `popup!` macro, which is expanded in other modules, so the context may be passed either
/// by value or by reference
pub fn push_message(
    ctx: impl std::borrow::Borrow<egui::Context>,
    message_type: PopupMessageType,
    message: impl ToString,
    time_left: Option<f32>,
    merge_with: Option<&str>,
) {
    let default_time_left = match message_type {
        PopupMessageType::Info => POPUP_MESSAGE_INFO_TIME,
        PopupMessageType::Warning => POPUP_MESSAGE_WARNING_TIME,
        PopupMessageType::Error => POPUP_MESSAGE_ERROR_TIME,
    };
    let message = PopupMessage {
        message_type,
        message: message.to_string(),
        time_left: time_left.unwrap_or(default_time_left),
        merge_with: merge_with.map(|key| key.to_string()),
        timestamp: std::time::Instant::now(),
    };

    // Appended to egui's memory, where the widget takes the messages from
    ctx.borrow().memory_mut(|w| {
        w.data
            .get_temp_mut_or_default::<Vec<PopupMessage>>(egui::Id::new("popup_messages"))
            .push(message);
    });
}

pub struct PopupWidget {
    messages: Vec<PopupMessage>,
    ctx: egui::Context,
}

impl PopupWidget {
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            messages: Vec::new(),
            ctx,
        }
    }

    fn merge_messages(&mut self) {
        let mut merged_messages: Vec<PopupMessage> = Vec::new();
        for message in self.messages.drain(..) {
            if let Some(merge_with) = &message.merge_with {
                if let Some(existing_message) = merged_messages
                    .iter_mut()
                    .find(|m| m.merge_with.as_ref() == Some(merge_with))
                {
                    // Messages with the same merge key share a single slot.
                    // The newest message (by timestamp) is the one which stays
                    if message.timestamp > existing_message.timestamp {
                        existing_message.message = message.message;
                        existing_message.time_left = message.time_left;
                        existing_message.message_type = message.message_type;
                    }
                    continue;
                }
            }
            merged_messages.push(message);
        }
        self.messages = merged_messages;
    }

    pub fn get_messages_from_ctx(&mut self) {
        // Taken out of egui's memory, otherwise every message would be received again
        // (and therefore duplicated) on each following frame
        let new_messages = self.ctx.memory_mut(|w| {
            w.data
                .remove_temp::<Vec<PopupMessage>>(egui::Id::new("popup_messages"))
                .unwrap_or_default()
        });
        self.messages.extend(new_messages);
        self.merge_messages();
    }
}

impl egui::Widget for &mut PopupWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        // Count the messages down and drop the ones which have finished sliding off
        let dt = ui.input(|input| input.stable_dt).min(0.1);
        for message in &mut self.messages {
            message.time_left -= dt;
        }
        self.messages.retain(|message| message.time_left > 0.0);

        if self.messages.is_empty() {
            return ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
        }

        // The countdown and the animations only advance while we repaint
        ui.ctx().request_repaint();

        // New messages are displayed at the top
        self.messages
            .sort_by_key(|message| std::cmp::Reverse(message.timestamp));

        let screen_rect = ui.ctx().content_rect();
        // Drawn above the rest of the app so the messages are always visible
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("popup_messages"),
        ));
        let font = egui::FontId::proportional(42.0);

        let mut top = screen_rect.top() + MESSAGE_EDGE_MARGIN;
        for message in &self.messages {
            let galley = painter.layout_no_wrap(
                message.message.clone(),
                font.clone(),
                MESSAGE_TEXT_COLOR,
            );
            let size = galley.size() + MESSAGE_PADDING * 2.0;
            let rect = egui::Rect::from_min_size(
                egui::Pos2::new(screen_rect.right() - MESSAGE_EDGE_MARGIN - size.x, top),
                size,
            );

            // The messages slide in from the right when they appear and slide off to the
            // right of the screen during the last part of their life
            let slide_distance = screen_rect.right() - rect.left() + MESSAGE_EDGE_MARGIN;
            let enter_progress =
                (message.timestamp.elapsed().as_secs_f32() / POPUP_MESSAGE_ANIMATION_TIME)
                    .clamp(0.0, 1.0);
            let exit_progress =
                1.0 - (message.time_left / POPUP_MESSAGE_ANIMATION_TIME).clamp(0.0, 1.0);
            let offset = (1.0 - back_out(enter_progress)) + back_in(exit_progress);
            let rect = rect.translate(egui::Vec2::new(offset * slide_distance, 0.0));

            // A semi transparent grey, tinted red for errors and yellow for warnings
            let fill_color = match &message.message_type {
                PopupMessageType::Info => MESSAGE_BACKGROUND_COLOR,
                PopupMessageType::Warning => MESSAGE_BACKGROUND_COLOR
                    .lerp_to_gamma(egui::Color32::from_rgb(255, 200, 0), MESSAGE_TINT_STRENGTH),
                PopupMessageType::Error => {
                    MESSAGE_BACKGROUND_COLOR.lerp_to_gamma(egui::Color32::RED, MESSAGE_TINT_STRENGTH)
                }
            }
            .gamma_multiply(MESSAGE_OPACITY);

            painter.rect_filled(rect, MESSAGE_CORNER_RADIUS, fill_color);
            painter.galley(rect.center() - galley.size() * 0.5, galley, MESSAGE_TEXT_COLOR);

            top += size.y + MESSAGE_EDGE_MARGIN;
        }

        ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
    }
}
