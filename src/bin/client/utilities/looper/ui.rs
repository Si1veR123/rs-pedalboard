use crate::{
    socket::{Command, LooperSubcommand},
    state::State,
    THEME_COLOR,
};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2, Widget};
use rs_pedalboard::looper_state::{LooperSettings, LooperSync};

const TITLE_SIZE: f32 = 24.0;
const SECTION_SIZE: f32 = 17.0;
const BODY_SIZE: f32 = 14.0;
const TRACK_CARD_SIZE: Vec2 = Vec2::new(132.0, 100.0);

pub struct LooperWidget {
    state: &'static State,
    new_multitrack_name: String,
}

impl LooperWidget {
    pub fn new(state: &'static State) -> Self {
        Self {
            state,
            new_multitrack_name: String::new(),
        }
    }

    fn send(&self, command: LooperSubcommand) {
        self.state
            .socket
            .borrow_mut()
            .send(Command::Looper(command));
    }

    fn send_settings(&self, settings: LooperSettings) {
        self.send(LooperSubcommand::SetSettings(settings));
    }

    fn status_button(
        ui: &mut egui::Ui,
        playing: bool,
        recording: bool,
        is_record: bool,
    ) -> egui::Response {
        let response = ui.add_sized(Vec2::splat(34.0), egui::Button::new(""));
        let color = if is_record && recording {
            Color32::from_rgb(220, 55, 45)
        } else if !is_record && playing {
            THEME_COLOR
        } else {
            Color32::from_gray(180)
        };
        let rect = response.rect.shrink(9.0);
        if is_record {
            ui.painter().circle_filled(rect.center(), 7.0, color);
        } else if playing {
            ui.painter().rect_filled(rect, 2.0, color);
        } else {
            ui.painter().add(egui::Shape::convex_polygon(
                vec![rect.left_top(), rect.right_center(), rect.left_bottom()],
                color,
                egui::Stroke::NONE,
            ));
        }
        response
    }

    fn draw_settings(&self, ui: &mut egui::Ui) {
        let information = self
            .state
            .looper_state
            .borrow()
            .get_looper_information()
            .cloned();
        let Some(information) = information else {
            ui.label(RichText::new("Loading looper settings...").color(Color32::from_gray(130)));
            return;
        };

        let mut settings = information.settings;
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Settings").size(SECTION_SIZE).strong());
                    ui.horizontal_centered(|ui| {
                        ui.label(RichText::new("Maximum fractionals").size(BODY_SIZE));
                        if ui
                            .add(
                                egui::DragValue::new(&mut settings.maximum_fractionals)
                                    .range(1..=64),
                            )
                            .changed()
                        {
                            self.state
                                .looper_state
                                .borrow_mut()
                                .set_settings(settings.clone());
                            self.send_settings(settings.clone());
                        }
                    });
                });

                let mut sync_enabled = settings.looper_sync.is_some();
                ui.horizontal(|ui| {
                    if ui
                        .checkbox(
                            &mut sync_enabled,
                            RichText::new("Quantize to sync").size(BODY_SIZE),
                        )
                        .changed()
                    {
                        settings.looper_sync = if sync_enabled {
                            Some(LooperSync {
                                bpm: 120,
                                beats_per_bar: 4,
                            })
                        } else {
                            None
                        };
                        self.state
                            .looper_state
                            .borrow_mut()
                            .set_settings(settings.clone());
                        self.send_settings(settings.clone());
                    }

                    if let Some(sync) = &mut settings.looper_sync {
                        let mut changed = false;
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut sync.bpm)
                                    .range(1..=999)
                                    .prefix("BPM "),
                            )
                            .changed();
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut sync.beats_per_bar)
                                    .range(1..=32)
                                    .prefix("Beats/bar "),
                            )
                            .changed();
                        if changed {
                            self.state
                                .looper_state
                                .borrow_mut()
                                .set_settings(settings.clone());
                            self.send_settings(settings);
                        }
                    }
                });
            });
    }
}

impl Widget for &mut LooperWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new("Looper").size(TITLE_SIZE).strong());
            self.draw_settings(ui);
            ui.add_space(10.0);
            ui.separator();

            let (active, library) = {
                let looper_state = self.state.looper_state.borrow();
                (
                    looper_state.active_multitrack().cloned(),
                    looper_state
                        .get_looper_information()
                        .map(|info| info.multitracks_information.clone())
                        .unwrap_or_default(),
                )
            };

            ui.add_space(10.0);
            ui.label(
                RichText::new("Active multitrack")
                    .size(SECTION_SIZE)
                    .strong(),
            );
            let has_active_multitrack = active.is_some();
            if let Some(active) = active {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&active.name).strong().size(BODY_SIZE));
                    if ui.button("Stop all").clicked() {
                        self.send(LooperSubcommand::StopAll);
                        self.state.looper_state.borrow_mut().stop_all_tracks();
                    }
                    if ui.button("Play all").clicked() {
                        self.send(LooperSubcommand::PlayAll);
                        self.state.looper_state.borrow_mut().play_all_tracks();
                    }
                });
                if active.track_count == 0 {
                    ui.label(
                        RichText::new("No tracks yet. Add a track below.")
                            .color(Color32::from_gray(140)),
                    );
                } else {
                    let columns = (((ui.available_width() + 12.0) / (TRACK_CARD_SIZE.x + 12.0))
                        as usize)
                        .max(1);
                    let rows = active.track_count.div_ceil(columns);
                    let grid_width =
                        columns as f32 * TRACK_CARD_SIZE.x + (columns - 1) as f32 * 12.0;
                    let grid_height =
                        rows as f32 * TRACK_CARD_SIZE.y + (rows.saturating_sub(1)) as f32 * 12.0;
                    ui.allocate_ui_with_layout(
                        Vec2::new(grid_width.min(ui.available_width()), grid_height),
                        egui::Layout::top_down(egui::Align::Center),
                        |ui| {
                            egui::Grid::new("looper_track_grid")
                                .num_columns(columns)
                                .spacing(Vec2::new(12.0, 12.0))
                                .show(ui, |ui| {
                                    for track_index in 0..active.track_count {
                                        let selected = active.selected_track == Some(track_index);
                                        let playing = active.tracks_playing.contains(&track_index);
                                        let recording = active.track_recording == Some(track_index);
                                        let fill = if selected {
                                            THEME_COLOR.gamma_multiply(0.35)
                                        } else {
                                            ui.visuals().widgets.inactive.bg_fill
                                        };
                                        ui.allocate_ui_with_layout(
                                            TRACK_CARD_SIZE,
                                            egui::Layout::top_down(egui::Align::Center),
                                            |ui| {
                                                egui::Frame::group(ui.style())
                                                    .fill(fill)
                                                    .stroke(Stroke::new(
                                                        1.0,
                                                        if selected {
                                                            THEME_COLOR
                                                        } else {
                                                            ui.visuals()
                                                                .widgets
                                                                .inactive
                                                                .bg_stroke
                                                                .color
                                                        },
                                                    ))
                                                    .inner_margin(8.0)
                                                    .show(ui, |ui| {
                                                        ui.vertical_centered(|ui| {
                                                            let track_response = ui.add(
                                                                egui::Button::new(
                                                                    RichText::new(format!(
                                                                        "Track {}",
                                                                        track_index + 1
                                                                    ))
                                                                    .strong(),
                                                                )
                                                                .fill(Color32::TRANSPARENT)
                                                                .stroke(egui::Stroke::NONE),
                                                            );
                                                            if track_response.clicked() {
                                                                self.state
                                                                    .looper_state
                                                                    .borrow_mut()
                                                                    .set_selected_track(Some(
                                                                        track_index,
                                                                    ));
                                                            }
                                                            ui.horizontal(|ui| {
                                                                let play_response =
                                                                    LooperWidget::status_button(
                                                                        ui, playing, recording,
                                                                        false,
                                                                    )
                                                                    .on_hover_text(if playing {
                                                                        "Stop playback"
                                                                    } else {
                                                                        "Start playback"
                                                                    });
                                                                if play_response.clicked() {
                                                                    self.send(if playing {
                                                                        LooperSubcommand::StopTrack(
                                                                            track_index,
                                                                        )
                                                                    } else {
                                                                        LooperSubcommand::PlayTrack(
                                                                            track_index,
                                                                        )
                                                                    });
                                                                    if playing {
                                                                        self.state
                                                                            .looper_state
                                                                            .borrow_mut()
                                                                            .stop_track(
                                                                                track_index,
                                                                            );
                                                                    } else {
                                                                        self.state
                                                                            .looper_state
                                                                            .borrow_mut()
                                                                            .play_track(
                                                                                track_index,
                                                                            );
                                                                    }
                                                                }
                                                                let record_response =
                                                                    LooperWidget::status_button(
                                                                        ui, playing, recording,
                                                                        true,
                                                                    )
                                                                    .on_hover_text(if recording {
                                                                        "Stop recording"
                                                                    } else {
                                                                        "Start recording"
                                                                    });
                                                                if record_response.clicked() {
                                                                    self.send(if recording {
                                                            LooperSubcommand::StopRecording
                                                        } else {
                                                            LooperSubcommand::StartRecording(
                                                                track_index,
                                                            )
                                                        });
                                                                    if recording {
                                                                        self.state
                                                                            .looper_state
                                                                            .borrow_mut()
                                                                            .stop_recording();
                                                                    } else {
                                                                        self.state
                                                                            .looper_state
                                                                            .borrow_mut()
                                                                            .set_recording_track(
                                                                                Some(track_index),
                                                                            );
                                                                    }
                                                                }
                                                            });
                                                        });
                                                    });
                                            },
                                        );
                                        if (track_index + 1) % columns == 0 {
                                            ui.end_row();
                                        }
                                    }
                                });
                        },
                    );
                }
            } else {
                ui.label(RichText::new("No active multitrack").color(Color32::from_gray(130)));
            }

            if has_active_multitrack {
                ui.add_space(8.0);
                if ui
                    .add_sized(
                        Vec2::new(150.0, 30.0),
                        egui::Button::new(RichText::new("Add track").size(BODY_SIZE)),
                    )
                    .clicked()
                {
                    self.send(LooperSubcommand::AddTrackToActiveMultitrack);
                    self.state
                        .looper_state
                        .borrow_mut()
                        .add_track_to_active_multitrack();
                }
            }

            ui.add_space(14.0);
            ui.label(
                RichText::new("Multitrack library")
                    .size(SECTION_SIZE)
                    .strong(),
            );
            ui.add_space(6.0);
            ui.horizontal_centered(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_multitrack_name)
                        .desired_width(220.0)
                        .hint_text("New multitrack name"),
                );
                if ui.button("Add multitrack").clicked()
                    && !self.new_multitrack_name.trim().is_empty()
                {
                    self.send(LooperSubcommand::AddMultitrack(
                        self.new_multitrack_name.trim().to_string(),
                    ));
                    self.state
                        .looper_state
                        .borrow_mut()
                        .add_multitrack(self.new_multitrack_name.trim().to_string());
                    self.new_multitrack_name.clear();
                }
            });
            egui::ScrollArea::vertical()
                .id_salt("looper_multitrack_library")
                .max_height(ui.available_height().max(160.0))
                .show(ui, |ui| {
                    for multitrack in library {
                        ui.horizontal_centered(|ui| {
                            let response = ui.add_sized(
                                Vec2::new(ui.available_width().min(420.0), 38.0),
                                egui::Button::new(
                                    RichText::new(format!(
                                        "{}  ({} tracks)",
                                        multitrack.name, multitrack.track_count
                                    ))
                                    .size(BODY_SIZE),
                                )
                                .corner_radius(8.0),
                            );
                            if response.clicked() {
                                self.send(LooperSubcommand::SetActiveMultitrack(
                                    multitrack.name.clone(),
                                ));
                                self.state
                                    .looper_state
                                    .borrow_mut()
                                    .set_active_multitrack(multitrack.name);
                            }
                        });
                    }
                });
            ui.add_space(8.0);
        })
        .response
    }
}
