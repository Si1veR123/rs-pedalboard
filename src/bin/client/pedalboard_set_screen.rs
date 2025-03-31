use eframe::egui::{self, Layout, Vec2, Widget};
use crate::state::State;


pub enum CurrentAction {
    Duplicate(usize),
    Remove(usize),
    SaveToSong(String),
    Rename((usize, String)),
    SaveToLibrary(usize)
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

    /// The panel that lists pedalboards in the set and various options
    /// TODO: Drag and drop to reorder pedalboards
    fn pedalboard_set_panel(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let mut active_pedalboards = self.state.active_pedalboardset.borrow_mut();
        let mut pedalboard_library = self.state.pedalboard_library.borrow_mut();

        // === Header buttons ===
        let buttons_row_height = 30.0;
        ui.columns(2, |columns| {
            columns[0].allocate_ui_with_layout(
                Vec2::new(0.0, buttons_row_height),
                Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(10.0);
                    if ui.add_sized([100.0, buttons_row_height], egui::Button::new("Clear Set")).clicked() {
                        active_pedalboards.pedalboards.clear();
                    }
                }
            );

            columns[1].allocate_ui_with_layout(
                Vec2::new(0.0, buttons_row_height),
                Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.add_space(10.0);
                    if ui.add_sized([100.0, buttons_row_height], egui::Button::new("Save to Song")).clicked() {
                        self.current_action = Some(CurrentAction::SaveToSong(String::new()));
                    }
                }
            );
        });

        // === Active Pedalboard Set List ===
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
                            // === Each Row ===
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

                                        ui.menu_button("...", |ui| {
                                            if ui.add(egui::Button::new("Remove From Set")).clicked() {
                                                self.current_action = Some(CurrentAction::Remove(i));
                                            }
    
                                            if ui.add(egui::Button::new("Rename")).clicked() {
                                                self.current_action = Some(CurrentAction::Rename((i, pedalboard.name.clone())));
                                            }
    
                                            if ui.add(egui::Button::new("Duplicate")).clicked() {
                                                self.current_action = Some(CurrentAction::Duplicate(i));
                                            }
                                        });

                                        ui.add_space(20.0);

                                        let in_library = pedalboard_library.iter().any(|library_pedalboard| library_pedalboard.name == pedalboard.name);
                                        if in_library {
                                            ui.add(egui::Button::new("Saved"));
                                        } else {
                                            if ui.add(egui::Button::new("Save")).clicked() {
                                                self.current_action = Some(CurrentAction::SaveToLibrary(i));
                                            }
                                        }
                                    }
                                )
                            })
                        }
                    );
                    ui.end_row();
                }
            }).response;

        // === Perform actions ===
        match self.current_action.take() {
            Some(CurrentAction::Duplicate(index)) => {
                let mut pedalboard = active_pedalboards.pedalboards.get(index).unwrap().clone();
                drop(active_pedalboards);
                drop(pedalboard_library);
                let unique_name = self.state.unique_pedalboard_name(pedalboard.name.clone());
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
                let saved = self.input_string_window(ui, "Rename", &mut new_name, &mut open);

                if saved {
                    let old_name = active_pedalboards.pedalboards.get(index).unwrap().name.clone();
                    drop(active_pedalboards);
                    drop(pedalboard_library);
                    let unique_name = self.state.unique_pedalboard_name(new_name.clone());
                    self.state.rename_pedalboard(&old_name, &unique_name);
                }

                if open {
                    self.current_action = Some(CurrentAction::Rename((index, new_name)));
                }
            },
            Some(CurrentAction::SaveToLibrary(index)) => {
                let pedalboard = active_pedalboards.pedalboards.get(index).unwrap().clone();
                pedalboard_library.push(pedalboard);
            },
            None => {}
        }

        response
    }
}

impl Widget for &mut PedalboardSetScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let width = ui.available_width();
        let height = ui.available_height();
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(width * 0.3, height),
                    Layout::top_down(egui::Align::Center),
                    |ui| self.pedalboard_set_panel(ui)
            );
            ui.allocate_ui_with_layout(
                Vec2::new(width * 0.7, height),
                Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, egui::Color32::from_white_alpha(255));
                    ui.label("Pedalboard");
            });
        }).response
    }
}
