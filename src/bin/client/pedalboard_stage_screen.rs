use std::cell::RefCell;
use std::rc::Rc;

use eframe::egui::{self, Color32, Layout, Rgba, RichText, Vec2, Widget};
use rs_pedalboard::pedalboard_set::PedalboardSet;
use crate::socket::ClientSocket;
use crate::state::State;
use crate::THEME_COLOUR;


pub enum CurrentAction {
    Duplicate(usize),
    Remove(usize),
    SaveToSong(String),
    Rename((usize, String)),
    SaveToLibrary(usize),
    ChangeActive(usize)
}

pub struct PedalboardstageScreen {
    state: &'static State,
    pub socket: Rc<RefCell<ClientSocket>>,
    current_action: Option<CurrentAction>,
}

impl PedalboardstageScreen {
    pub fn new(state: &'static State, socket: Rc<RefCell<ClientSocket>>) -> Self {
        Self {
            state,
            socket,
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

    /// The panel that lists pedalboards in the stage and various options
    /// TODO: Drag and drop to reorder pedalboards
    fn pedalboard_stage_panel(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let mut active_pedalboards = self.state.active_pedalboardstage.borrow_mut();
        let mut pedalboard_library = self.state.pedalboard_library.borrow_mut();

        ui.add_space(5.0);

        // === Header buttons ===
        let buttons_row_height = 50.0;
        ui.columns(2, |columns| {
            columns[0].allocate_ui_with_layout(
                Vec2::new(0.0, buttons_row_height),
                Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(10.0);
                    if ui.add_sized([100.0, buttons_row_height], egui::Button::new("Clear Stage")).clicked() {
                        *active_pedalboards = PedalboardSet::default();
                        self.socket.borrow_mut().load_set(&active_pedalboards);
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

        ui.add_space(5.0);
        ui.separator();
        ui.add_space(5.0);

        // === Active Pedalboard stage List ===
        let row_width = ui.available_width();
        let row_height = 50.0;

        let response = egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("pedalboard_stage_grid")
                .with_row_color(|index, _style| {
                    if index % 2 == 0 {
                        Some(crate::ROW_COLOUR_DARK)
                    } else {
                        Some(crate::ROW_COLOUR_LIGHT)
                    }
                })
                .show(ui, |ui| {
                    for (i, pedalboard) in active_pedalboards.pedalboards.iter().enumerate() {
                        ui.allocate_ui_with_layout(
                            Vec2::new(row_width, row_height),
                            Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                // === Each Row ===
                                ui.set_min_size(Vec2::new(row_width, row_height));

                                if ui.interact(ui.available_rect_before_wrap(), egui::Id::new(i), egui::Sense::CLICK).clicked() {
                                    self.current_action = Some(CurrentAction::ChangeActive(i));
                                }

                                if active_pedalboards.active_pedalboard == i {
                                    ui.painter().rect_filled(
                                        ui.available_rect_before_wrap(),
                                        5.0,
                                        Color32::from(THEME_COLOUR.linear_multiply(0.05))
                                    );
                                }
                                
                                // TODO: Fix pedalboard name column can't take up more than 50% of the row
                                ui.columns(2, |columns| {
                                    columns[0].horizontal_centered(|ui| {
                                        ui.add_space(20.0);
                                        let text_color = if active_pedalboards.active_pedalboard == i {
                                            Rgba::from_white_alpha(0.9)
                                        } else {
                                            Rgba::from_white_alpha(0.4)
                                        };
                                        if ui.label(RichText::new(pedalboard.name.clone()).color(text_color).size(20.0)).clicked() {
                                            self.current_action = Some(CurrentAction::ChangeActive(i));
                                        }
                                    });

                                    columns[1].allocate_ui_with_layout(
                                        Vec2::new(0.0, row_height),
                                        Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.add_space(20.0);

                                            ui.menu_button("...", |ui| {
                                                ui.add_space(5.0);
                                                if ui.add(egui::Button::new(RichText::new("Remove From Stage").size(25.0))).clicked() {
                                                    self.current_action = Some(CurrentAction::Remove(i));
                                                }
                                                ui.add_space(2.0);
                                                ui.separator();
                                                ui.add_space(2.0);
                                                if ui.add(egui::Button::new(RichText::new("Rename").size(25.0))).clicked() {
                                                    self.current_action = Some(CurrentAction::Rename((i, pedalboard.name.clone())));
                                                }
                                                ui.add_space(2.0);
                                                ui.separator();
                                                ui.add_space(2.0);
                                                if ui.add(egui::Button::new(RichText::new("Duplicate").size(25.0))).clicked() {
                                                    self.current_action = Some(CurrentAction::Duplicate(i));
                                                }
                                                ui.add_space(5.0);
                                            });

                                            ui.add_space(5.0);

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
                }).response
        }).inner;

        // === Perform actions ===
        match self.current_action.take() {
            Some(CurrentAction::Duplicate(index)) => {
                let mut pedalboard = active_pedalboards.pedalboards.get(index).unwrap().clone();
                let pedalboardset_length = active_pedalboards.pedalboards.len();
                drop(active_pedalboards);
                drop(pedalboard_library);
                let unique_name = self.state.unique_pedalboard_name(pedalboard.name.clone());
                pedalboard.name = unique_name;

                let mut socket = self.socket.borrow_mut();
                socket.add_pedalboard(&pedalboard).expect("Failed to add pedalboard");
                socket.move_pedalboard(pedalboardset_length-1, index+1).expect("Failed to move pedalboard");

                self.state.active_pedalboardstage.borrow_mut().pedalboards.insert(index+1, pedalboard);
            },
            Some(CurrentAction::Remove(index)) => {
                active_pedalboards.remove_pedalboard(index);

                
                let mut socket = self.socket.borrow_mut();
                socket.delete_pedalboard(index).expect("Failed to remove pedalboard");
            },
            Some(CurrentAction::SaveToSong(mut song_name)) => {
                let mut open = true;
                drop(active_pedalboards);
                let saved = self.input_string_window(ui, "Save to Song", &mut song_name, &mut open);

                if saved {
                    let active_pedalboards = &self.state.active_pedalboardstage.borrow().pedalboards;
                    // Save all pedalboards in library, if they arent saved
                    for pedalboard in active_pedalboards.iter() {
                        let in_library = pedalboard_library.iter().any(|library_pedalboard| library_pedalboard.name == pedalboard.name);
                        if !in_library {
                            pedalboard_library.push(pedalboard.clone());
                        }
                    }

                    let pedalboard_names: Vec<String> = active_pedalboards.iter()
                        .map(|pedalboard| pedalboard.name.clone())
                        .collect();
                    self.state.songs_library.borrow_mut().insert(song_name.clone(), pedalboard_names);
                } else if open {
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
            Some(CurrentAction::ChangeActive(index)) => {
                active_pedalboards.active_pedalboard = index;

                let mut socket = self.socket.borrow_mut();
                socket.play(index).expect("Failed to change active pedalboard");
            },
            None => {}
        }

        response
    }
}

impl Widget for &mut PedalboardstageScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let width = ui.available_width();
        let height = ui.available_height();
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(width * 0.4, height),
                    Layout::top_down(egui::Align::Center),
                    |ui| self.pedalboard_stage_panel(ui)
            );
            ui.allocate_ui_with_layout(
                Vec2::new(width * 0.6, height),
                Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, egui::Color32::BLACK);
                    ui.label("Pedalboard");
            });
        }).response
    }
}
