use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Layout, TextEdit, Vec2, Widget};
use rs_pedalboard::pedalboard::Pedalboard;
use crate::{helpers::unique_pedalboard_name, State};

pub struct PedalboardListScreen {
    // Store pedalboards by unique name
    state: &'static State,
    search_term: String,
}

impl PedalboardListScreen {
    pub fn new(state: &'static State) -> Self {
        Self {
            state,
            search_term: String::new(),
        }
    }
}

impl Widget for &mut PedalboardListScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.add_space(10.0);

        ui.columns(3, |columns| {
            columns[1].add_sized([0.0, 30.0], TextEdit::singleline(&mut self.search_term).hint_text("Search..."));


            columns[2].allocate_ui_with_layout(
                Vec2::new(0.0, 30.0),
                Layout::top_down(egui::Align::Center),
                |ui| {
                    if ui.add_sized([200.0, 30.0], egui::Button::new("New Pedalboard")).clicked() {
                        let mut pedalboards_mut = self.state.pedalboard_library.borrow_mut();
                        let unique_name = unique_pedalboard_name(String::from("New Pedalboard"), pedalboards_mut.as_ref());
                        pedalboards_mut.push(Pedalboard::new(unique_name));
                }
            });
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        let pedalboard_library = self.state.pedalboard_library.borrow();
        let row_height = 30.0;
        let row_size = Vec2::new(ui.available_width(), row_height);

        if pedalboard_library.is_empty() {
            ui.add_sized(row_size, egui::Label::new("No pedalboards found"))
        } else {
            let mut action_pedalboard = None;
            let mut action_is_delete = false;

            let response = egui::Grid::new("pedalboard_list_grid")
                .striped(true)
                .spacing(Vec2::new(10.0, 10.0))
                .show(ui, |ui| {
                    for pedalboard in pedalboard_library.iter() {
                        if self.search_term.is_empty() || pedalboard.name.contains(&self.search_term) {

                            // EACH PEDALBOARD ROW
                            ui.allocate_ui_with_layout(
                                row_size,
                                Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_min_size(row_size);
                                    ui.columns(2, |columns| {
                                        columns[0].label(&pedalboard.name);
                                        columns[1].allocate_ui_with_layout(
                                            Vec2::new(0.0, row_height),
                                            Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.button("Delete").clicked() {
                                                    action_pedalboard = Some(pedalboard.name.clone());
                                                    action_is_delete = true;
                                                }
                                                if ui.button("Load").clicked() {
                                                    action_pedalboard = Some(pedalboard.name.clone());
                                                    action_is_delete = false;
                                                }
                                            }
                                        )
                                    });
                            });
                            ui.end_row();
                        }
                    }
            }).response;

            drop(pedalboard_library);

            if let Some(pedalboard_name) = action_pedalboard {
                let mut pedalboards_mut = self.state.pedalboard_library.borrow_mut();
                if action_is_delete {
                    pedalboards_mut.retain(|p| p.name != pedalboard_name);
                } else {
                    let pedalboard = pedalboards_mut.iter().find(|p| p.name == pedalboard_name).unwrap();
                    self.state.active_pedalboardset.borrow_mut().pedalboards.push(pedalboard.clone());
                }
            };

            response
        }
    }
}