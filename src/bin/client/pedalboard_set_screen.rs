use eframe::egui::{self, Layout, Vec2, Widget};
use crate::helpers::unique_pedalboard_name;

use crate::State;

pub enum CurrentAction {
    Duplicate(usize),
    Remove(usize),
    SaveToSong(String),
    Rename((usize, String))
}

pub struct PedalboardSetScreen {
    state: &'static State,
    current_action: Option<CurrentAction>,
}

impl PedalboardSetScreen {
    pub fn new(state: &'static State) -> Self {
        Self {
            state,
            current_action: None
        }
    }

    fn input_string_window(&mut self, ui: &mut egui::Ui, title: &str, input: &mut String, open: &mut bool) -> bool {
        let mut saved = false;
        egui::Window::new(title)
            .open(open)
            .show(ui.ctx(), |ui| {
                ui.add(egui::TextEdit::singleline(input));
                if ui.button("Save").clicked() {
                    saved = true;
                }
            });

        if saved {
            *open = false;
        }

        saved
    }

    fn pedalboard_set_panel(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let mut active_pedalboards = self.state.active_pedalboardset.borrow_mut();

        let buttons_row_height = 30.0;
        ui.columns(2, |columns| {
            columns[0].allocate_ui_with_layout(
                Vec2::new(0.0, buttons_row_height),
                Layout::left_to_right(egui::Align::Center),
                |ui| {
                    if ui.add_sized([100.0, buttons_row_height], egui::Button::new("Clear Set")).clicked() {
                        active_pedalboards.pedalboards.clear();
                    }
                }
            );

            columns[1].allocate_ui_with_layout(
                Vec2::new(0.0, buttons_row_height),
                Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if ui.add_sized([100.0, buttons_row_height], egui::Button::new("Save to Song")).clicked() {
                        self.current_action = Some(CurrentAction::SaveToSong(String::new()));
                    }
                }
            );
        });

        let row_width = ui.available_width();
        let row_height = 50.0;
        let response = egui::Grid::new("pedalboard_set_grid")
            .striped(true)
            .show(ui, |ui| {
                for (i, pedalboard) in active_pedalboards.pedalboards.iter().enumerate() {
                    ui.allocate_ui_with_layout(
                        Vec2::new(row_width, row_height),
                        Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_size(Vec2::new(row_width, row_height));
                            ui.columns(2, |columns| {
                                columns[0].horizontal_centered(|ui| {
                                    ui.add_space(20.0);
                                    ui.label(pedalboard.name.clone());
                                });

                                columns[1].allocate_ui_with_layout(
                                    Vec2::new(0.0, row_height),
                                    Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.add_space(20.0);

                                        if ui.add(egui::Button::new("Remove")).clicked() {
                                            self.current_action = Some(CurrentAction::Remove(i));
                                        }

                                        ui.add_space(10.0);

                                        if ui.add(egui::Button::new("Rename")).clicked() {
                                            self.current_action = Some(CurrentAction::Rename((i, pedalboard.name.clone())));
                                        }

                                        ui.add_space(10.0);

                                        if ui.add(egui::Button::new("Duplicate")).clicked() {
                                            self.current_action = Some(CurrentAction::Duplicate(i));
                                        }
                                    }
                                )
                            })
                        }
                    );
                    ui.end_row();
                }
            }).response;


        match self.current_action.take() {
            Some(CurrentAction::Duplicate(index)) => {
                let mut pedalboard = active_pedalboards.pedalboards.get(index).unwrap().clone();
                drop(active_pedalboards);
                let unique_name = unique_pedalboard_name(pedalboard.name.clone(), self.state);
                pedalboard.name = unique_name;
                self.state.active_pedalboardset.borrow_mut().pedalboards.insert(index+1, pedalboard);
            },
            Some(CurrentAction::Remove(index)) => {
                active_pedalboards.pedalboards.remove(index);
            },
            Some(CurrentAction::SaveToSong(mut song_name)) => {
                let mut open = true;
                drop(active_pedalboards);
                let saved = self.input_string_window(ui, "Save to Song", &mut song_name, &mut open);

                if saved {
                    let active_pedalboards = &self.state.active_pedalboardset.borrow().pedalboards;
                    let pedalboard_names: Vec<String> = active_pedalboards.iter()
                        .map(|pedalboard| pedalboard.name.clone())
                        .collect();
                    self.state.songs_library.borrow_mut().insert(song_name.clone(), pedalboard_names);
                }

                if open {
                    self.current_action = Some(CurrentAction::SaveToSong(song_name));
                }
            },
            Some(CurrentAction::Rename((index, mut new_name))) => {
                let mut open = true;
                drop(active_pedalboards);
                let saved = self.input_string_window(ui, "Rename", &mut new_name, &mut open);

                if saved {
                    let unique_name = unique_pedalboard_name(new_name.clone(), self.state);
                    let active_pedalboards = &mut self.state.active_pedalboardset.borrow_mut().pedalboards;
                    active_pedalboards.get_mut(index).unwrap().name = unique_name;
                }

                if open {
                    self.current_action = Some(CurrentAction::Rename((index, new_name)));
                }
            },
            None => {}
        }

        response
    }
}

impl Widget for &mut PedalboardSetScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.pedalboard_set_panel(ui)
    }
}
