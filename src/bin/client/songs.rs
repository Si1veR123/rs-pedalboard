use eframe::egui::{self, Layout, RichText, TextEdit, Vec2, Widget};

use crate::State;

pub enum RowAction {
    Load,
    Delete
}

pub struct SongsScreen {
    state: &'static State,
    search_term: String,
}

impl SongsScreen {
    pub fn new(state: &'static State) -> Self {
        Self {
            state,
            search_term: String::new()
        }
    }

    pub fn songs_row(ui: &mut egui::Ui, song_name: &str, pedalboards: &[String], row_size: Vec2) -> (Option<RowAction>, egui::Response) {
        let mut action = None;

        let row_height = row_size.y;
        let response = ui.allocate_ui_with_layout(
            row_size,
            Layout::top_down_justified(egui::Align::Center),
            |ui| {
                ui.columns(2, |columns| {
                    columns[0].allocate_ui_with_layout(
                        Vec2::new(0.0, row_height-20.0),
                        Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.add_space(20.0);
                            ui.label(RichText::new(song_name).size(20.0));   
                        }
                    );

                    columns[1].allocate_ui_with_layout(
                        Vec2::new(0.0, row_height-20.0),
                        Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.add_space(20.0);
                            if ui.add_sized([80.0, 30.0], egui::Button::new("Delete")).clicked() {
                                action = Some(RowAction::Delete);
                            }
                            if ui.add_sized([80.0, 30.0], egui::Button::new("Load")).clicked() {
                                action = Some(RowAction::Load);
                            }
                        }
                    )
                });

                let pedalboards_text = pedalboards.join(", ");
                ui.label(RichText::new(pedalboards_text).size(15.0));
                ui.add_space(5.0);
        }).response;

        (action, response)
    }
}

impl Widget for &mut SongsScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.add_space(10.0);

        ui.vertical_centered(|ui| {
            // === Search bar ===
            ui.add_sized(
                [ui.available_width()/3.0, 30.0],
                TextEdit::singleline(&mut self.search_term).hint_text(RichText::new("Search songs...").size(20.0))
            );

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // === Songs Grid ===
            let row_height = 70.0;
            let row_size = Vec2::new(ui.available_width(), row_height);

            let mut songs_library = self.state.songs_library.borrow_mut();
            if songs_library.is_empty() {
                ui.add_sized(row_size, egui::Label::new(RichText::new("No Songs Found").size(30.0)));
            } else {
                let mut action = None;

                egui::Grid::new("songs_library_grid")
                    .striped(true)
                    .spacing(Vec2::new(10.0, 10.0))
                    .show(ui, |ui| {
                        for (song, pedalboards) in songs_library.iter() {
                            if self.search_term.is_empty() || song.contains(&self.search_term) {
                                SongsScreen::songs_row(ui, song, pedalboards, row_size).0.map(|row_action| {
                                    action = Some((song, row_action));
                                });
                                ui.end_row();
                            }
                        }
                });

                // Perform any actions performed in this frame
                if let Some((song, action)) = action {
                    match action {
                        RowAction::Load => {
                            let song = songs_library.get(song).unwrap();
                            let pedalboard_library = self.state.pedalboard_library.borrow();
                            for pedalboard_name in song {
                                if let Some(pedalboard) = pedalboard_library.iter().find(|pedalboard| &pedalboard.name == pedalboard_name) {
                                    self.state.active_pedalboardstage.borrow_mut().pedalboards.push(pedalboard.clone());
                                    self.state.load_active_set();
                                }
                            }
                        },
                        RowAction::Delete => {
                            // Can't get from the hashmap with a reference to the String key
                            let cloned = song.clone();
                            songs_library.remove(&cloned);
                        }
                    }
                };
            }
        }).response
    }
}