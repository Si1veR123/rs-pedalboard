use crate::{
    socket::{Command, LooperSubcommand},
    state::State,
    ui_helpers::integer_selector,
    utilities::{BODY_FONT_SIZE, HEADING_FONT_SIZE},
    FAINT_TEXT_COLOR, THEME_COLOR,
};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2, Widget};
use rs_pedalboard::looper_state::{LooperSettings, LooperSync};

const TRACK_CARD_SIZE: Vec2 = Vec2::new(120.0, 120.0);

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
        self.state
            .socket
            .borrow_mut()
            .send(Command::Looper(LooperSubcommand::SetSettings(settings)));
    }

    fn setting_changed(&self, settings: LooperSettings) {
        self.state
            .looper_state
            .borrow_mut()
            .set_settings(settings.clone());
        self.send_settings(settings);
    }

    fn status_button(
        ui: &mut egui::Ui,
        active: bool,
        recording: bool,
        record_button: bool,
    ) -> egui::Response {
        let response = ui.add_sized(Vec2::splat(46.0), egui::Button::new(""));
        let color = if record_button && recording {
            Color32::from_rgb(220, 55, 45)
        } else if !record_button && active {
            THEME_COLOR
        } else {
            Color32::from_gray(175)
        };
        let icon_rect = response.rect.shrink(12.0);

        if record_button {
            ui.painter().circle_filled(icon_rect.center(), 10.0, color);
        } else if active {
            ui.painter().rect_filled(icon_rect, 2.0, color);
        } else {
            ui.painter().add(egui::Shape::convex_polygon(
                vec![
                    icon_rect.left_top(),
                    icon_rect.right_center(),
                    icon_rect.left_bottom(),
                ],
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
            ui.label(
                RichText::new("Loading settings…")
                    .size(BODY_FONT_SIZE)
                    .color(Color32::from_gray(140)),
            );
            return;
        };

        let mut settings = information.settings;
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width() * 0.75, 30.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                const SETTINGS_VERTICAL_SPACING: f32 = 25.0;

                ui.label(RichText::new("Settings").size(BODY_FONT_SIZE));

                ui.add_space(SETTINGS_VERTICAL_SPACING);

                let mut maximum_fractionals = settings.maximum_fractionals as u32;

                ui.label(
                    RichText::new(format!("Maximum Fractionals: {}", maximum_fractionals))
                        .size(BODY_FONT_SIZE),
                );

                if integer_selector(ui, &mut maximum_fractionals, 1..=64) {
                    settings.maximum_fractionals = maximum_fractionals as usize;
                    self.setting_changed(settings.clone());
                }

                ui.add_space(SETTINGS_VERTICAL_SPACING);
                let mut sync_enabled = settings.looper_sync.is_some();
                if ui
                    .checkbox(
                        &mut sync_enabled,
                        RichText::new("Quantize to BPM").size(BODY_FONT_SIZE),
                    )
                    .changed()
                {
                    settings.looper_sync = sync_enabled.then_some(LooperSync {
                        bpm: 120,
                        beats_per_bar: 4,
                    });
                    self.setting_changed(settings.clone());
                }

                let sync_values = settings
                    .looper_sync
                    .as_ref()
                    .map(|sync| (sync.bpm, sync.beats_per_bar));
                if let Some((bpm_value, beats_value)) = sync_values {
                    ui.add_space(SETTINGS_VERTICAL_SPACING);
                    let mut bpm = bpm_value as u32;

                    ui.label(RichText::new(format!("{} BPM", bpm)).size(BODY_FONT_SIZE));

                    if integer_selector(ui, &mut bpm, 1..=999) {
                        if let Some(sync) = settings.looper_sync.as_mut() {
                            sync.bpm = bpm as u16;
                        }
                        self.setting_changed(settings.clone());
                    }

                    ui.add_space(SETTINGS_VERTICAL_SPACING);
                    let mut beats_per_bar = beats_value as u32;

                    ui.label(
                        RichText::new(format!("{} Beats per Bar", beats_per_bar))
                            .size(BODY_FONT_SIZE),
                    );

                    if integer_selector(ui, &mut beats_per_bar, 1..=32) {
                        if let Some(sync) = settings.looper_sync.as_mut() {
                            sync.beats_per_bar = beats_per_bar as u8;
                        }
                        self.setting_changed(settings.clone());
                    }
                }
            },
        );
    }

    fn draw_track(
        &self,
        ui: &mut egui::Ui,
        track_index: usize,
        selected: bool,
        playing: bool,
        recording: bool,
    ) {
        let fill = if selected {
            THEME_COLOR.gamma_multiply(0.28)
        } else {
            ui.visuals().widgets.inactive.bg_fill
        };
        let stroke = if selected {
            THEME_COLOR
        } else {
            ui.visuals().widgets.inactive.bg_stroke.color
        };

        ui.allocate_ui_with_layout(
            TRACK_CARD_SIZE,
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                egui::Frame::group(ui.style())
                    .fill(fill)
                    .stroke(Stroke::new(1.0, stroke))
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.set_min_size(TRACK_CARD_SIZE - Vec2::splat(20.0));
                        ui.vertical_centered(|ui| {
                            let title = ui.add(
                                egui::Button::new(
                                    RichText::new(format!("{}", track_index + 1))
                                        .size(BODY_FONT_SIZE)
                                        .strong(),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE),
                            );
                            if title.clicked() {
                                self.state
                                    .looper_state
                                    .borrow_mut()
                                    .set_selected_track(Some(track_index));
                            }

                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                let play = Self::status_button(ui, playing, recording, false)
                                    .on_hover_text(if playing {
                                        "Stop playback"
                                    } else {
                                        "Start playback"
                                    });
                                if play.clicked() {
                                    self.send(if playing {
                                        LooperSubcommand::StopTrack(track_index)
                                    } else {
                                        LooperSubcommand::PlayTrack(track_index)
                                    });
                                    if playing {
                                        self.state
                                            .looper_state
                                            .borrow_mut()
                                            .stop_track(track_index);
                                    } else {
                                        self.state
                                            .looper_state
                                            .borrow_mut()
                                            .play_track(track_index);
                                    }
                                }

                                let record = Self::status_button(ui, playing, recording, true)
                                    .on_hover_text(if recording {
                                        "Stop recording"
                                    } else {
                                        "Start recording"
                                    });
                                if record.clicked() {
                                    self.send(if recording {
                                        LooperSubcommand::StopRecording
                                    } else {
                                        LooperSubcommand::StartRecording(track_index)
                                    });
                                    if recording {
                                        self.state.looper_state.borrow_mut().stop_recording();
                                    } else {
                                        self.state
                                            .looper_state
                                            .borrow_mut()
                                            .set_recording_track(Some(track_index));
                                    }
                                }
                            });
                        });
                    });
            },
        );
    }
}

impl Widget for &mut LooperWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::new("Looper").size(HEADING_FONT_SIZE));
            ui.add_space(10.0);
            self.draw_settings(ui);

            ui.add(egui::Separator::default().spacing(24.0).shrink(50.0));

            let (active, library) = {
                let state = self.state.looper_state.borrow();
                (
                    state.active_multitrack().cloned(),
                    state
                        .get_looper_information()
                        .map(|info| info.multitracks_information.clone())
                        .unwrap_or_default(),
                )
            };

            ui.label(
                RichText::new("Active Multitrack")
                    .size(BODY_FONT_SIZE)
                    .strong(),
            );
            ui.add_space(6.0);

            if let Some(active) = active {
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width() * 0.4, 30.0),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.label(RichText::new(active.name.clone()).size(BODY_FONT_SIZE));
                        let playback_button_size = Vec2::new(ui.available_width(), 44.0);
                        if ui
                            .add_sized(
                                playback_button_size,
                                egui::Button::new(RichText::new("Play all").size(BODY_FONT_SIZE)),
                            )
                            .clicked()
                        {
                            self.send(LooperSubcommand::PlayAll);
                            self.state.looper_state.borrow_mut().play_all_tracks();
                        }

                        ui.add_space(10.0);

                        if ui
                            .add_sized(
                                playback_button_size,
                                egui::Button::new(RichText::new("Stop all").size(BODY_FONT_SIZE)),
                            )
                            .clicked()
                        {
                            self.send(LooperSubcommand::StopAll);
                            self.state.looper_state.borrow_mut().stop_all_tracks();
                        }

                        ui.add_space(10.0);

                        if ui
                            .add_sized(
                                playback_button_size,
                                egui::Button::new(
                                    RichText::new("Stop all immediately").size(BODY_FONT_SIZE),
                                ),
                            )
                            .clicked()
                        {
                            self.send(LooperSubcommand::StopAllImmediately);
                            self.state.looper_state.borrow_mut().stop_all_tracks();
                        }
                    },
                );
                ui.add_space(10.0);

                if active.track_count == 0 {
                    ui.label(
                        RichText::new("No tracks")
                            .size(BODY_FONT_SIZE)
                            .color(Color32::from_gray(150)),
                    );
                } else {
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width() * 0.75, TRACK_CARD_SIZE.y),
                        egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
                        |ui| {
                            for index in 0..active.track_count {
                                self.draw_track(
                                    ui,
                                    index,
                                    active.selected_track == Some(index),
                                    active.tracks_playing.contains(&index),
                                    active.track_recording == Some(index),
                                );
                                ui.add_space(8.0);
                            }
                        },
                    );
                }

                ui.add_space(12.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width() * 0.75, 44.0),
                    egui::Layout::left_to_right(egui::Align::Min),
                    |ui| {
                        ui.columns(6, |columns| {
                            if columns[0].button("Add track").clicked() {
                                self.send(LooperSubcommand::AddTrackToActiveMultitrack);
                                self.state
                                    .looper_state
                                    .borrow_mut()
                                    .add_track_to_active_multitrack();
                            }

                            if columns[1].button("Remove track").clicked() {
                                if let Some(index) = active.selected_track {
                                    self.send(LooperSubcommand::RemoveTrackAt(index));
                                }
                                self.state.looper_state.borrow_mut().remove_selected_track();
                            }

                            if columns[2].button("Clear track").clicked() {
                                if let Some(index) = active.selected_track {
                                    self.send(LooperSubcommand::ClearTrackAt(index));
                                }
                                self.state.looper_state.borrow_mut().clear_selected_track();
                            }

                            if columns[3].button("Half Speed FX").clicked() {
                                if let Some(index) = active.selected_track {
                                    self.send(LooperSubcommand::HalfSpeedEffectAt(index));
                                }
                            }

                            if columns[4].button("Double Speed FX").clicked() {
                                if let Some(index) = active.selected_track {
                                    self.send(LooperSubcommand::DoubleSpeedEffectAt(index));
                                }
                            }

                            if columns[5].button("Reverse FX").clicked() {
                                if let Some(index) = active.selected_track {
                                    self.send(LooperSubcommand::ReverseEffectAt(index));
                                }
                            }
                        });
                    },
                );
            } else {
                ui.label(
                    RichText::new("No active multitrack")
                        .size(BODY_FONT_SIZE)
                        .color(Color32::from_gray(150)),
                );
            }

            ui.add(egui::Separator::default().spacing(24.0).shrink(50.0));

            ui.label(
                RichText::new("Multitrack library")
                    .size(BODY_FONT_SIZE)
                    .strong(),
            );
            ui.add_space(6.0);
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), 30.0),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.add_sized(
                        Vec2::new(ui.available_width() * 0.5, 44.0),
                        egui::TextEdit::singleline(&mut self.new_multitrack_name)
                            .hint_text("New multitrack name"),
                    );
                    ui.add_space(8.0);
                    if ui
                        .add_sized(
                            Vec2::new(ui.available_width() * 0.5, 44.0),
                            egui::Button::new(RichText::new("Add multitrack").size(BODY_FONT_SIZE)),
                        )
                        .clicked()
                        && !self.new_multitrack_name.trim().is_empty()
                    {
                        let name = self.new_multitrack_name.trim().to_string();
                        self.send(LooperSubcommand::AddMultitrack(name.clone()));
                        self.state.looper_state.borrow_mut().add_multitrack(name);
                        self.new_multitrack_name.clear();
                    }
                },
            );

            ui.add_space(20.0);

            if library.is_empty() {
                ui.label(
                    RichText::new("No multitracks available")
                        .size(BODY_FONT_SIZE)
                        .color(FAINT_TEXT_COLOR),
                );
            } else {
                for multitrack in library {
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), 44.0),
                        egui::Layout::top_down(egui::Align::Center),
                        |ui| {
                            ui.allocate_ui_with_layout(
                                Vec2::new(ui.available_width() * 0.5, 44.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    let selected = ui
                                        .add_sized(
                                            Vec2::new(ui.available_width() - 90.0, 44.0),
                                            egui::Button::new(
                                                RichText::new(format!(
                                                    "{}  -  {} tracks",
                                                    multitrack.name, multitrack.track_count
                                                ))
                                                .size(BODY_FONT_SIZE),
                                            ),
                                        )
                                        .clicked();
                                    if selected {
                                        self.send(LooperSubcommand::SetActiveMultitrack(
                                            multitrack.name.clone(),
                                        ));
                                        self.state
                                            .looper_state
                                            .borrow_mut()
                                            .set_active_multitrack(multitrack.name.clone());
                                    }
                                    if ui
                                        .add_sized(
                                            Vec2::new(80.0, 44.0),
                                            egui::Button::new("Delete"),
                                        )
                                        .clicked()
                                    {
                                        self.send(LooperSubcommand::DeleteMultitrack(
                                            multitrack.name.clone(),
                                        ));
                                        self.state
                                            .looper_state
                                            .borrow_mut()
                                            .delete_multitrack(&multitrack.name);
                                    }
                                },
                            );
                        },
                    );
                    ui.add_space(4.0);
                }
            }
            ui.add_space(10.0);
        })
        .response
    }
}
