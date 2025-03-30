use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Layout, RichText, TextEdit, Vec2, Widget};
use rs_pedalboard::pedalboard::Pedalboard;
use crate::{helpers::unique_pedalboard_name, State};

pub enum RowAction {
    Load,
    Delete
}

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

    pub fn pedalboard_row(ui: &mut egui::Ui, pedalboard: &Pedalboard, row_size: Vec2) -> (Option<RowAction>, egui::Response) {
        let mut action = None;

        let row_height = row_size.y;
        let response = ui.allocate_ui_with_layout(
            row_size,
            Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_size(row_size);
                ui.columns(2, |columns| {
                    columns[0].horizontal_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(RichText::new(&pedalboard.name).size(20.0));   
                    });

                    columns[1].allocate_ui_with_layout(
                        Vec2::new(0.0, row_height),
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
        }).response;

        (action, response)
    }
}

impl Widget for &mut PedalboardListScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.add_space(10.0);

        ui.columns(3, |columns| {
            columns[1]
                .add_sized(
                    [0.0, 30.0],
                    TextEdit::singleline(&mut self.search_term).hint_text(RichText::new("Search pedalboards...").size(20.0))
                );


            columns[2].allocate_ui_with_layout(
                Vec2::new(0.0, 30.0),
                Layout::top_down(egui::Align::Center),
                |ui| {
                    if ui.add_sized([200.0, 30.0], egui::Button::new("New Pedalboard")).clicked() {
                        let unique_name = unique_pedalboard_name(String::from("New Pedalboard"), self.state);
                        let pedalboards_mut = &mut self.state.pedalboard_library.borrow_mut();
                        pedalboards_mut.push(Pedalboard::new(unique_name));
                }
            });
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        let pedalboard_library = self.state.pedalboard_library.borrow();
        let row_height = 50.0;
        let row_size = Vec2::new(ui.available_width(), row_height);

        if pedalboard_library.is_empty() {
            ui.add_sized(row_size, egui::Label::new(RichText::new("No Pedalboards Found").size(30.0)))
        } else {
            let mut action = None;

            let response = egui::Grid::new("pedalboard_list_grid")
                .striped(true)
                .spacing(Vec2::new(10.0, 10.0))
                .show(ui, |ui| {
                    for (i, pedalboard) in pedalboard_library.iter().enumerate() {
                        if self.search_term.is_empty() || pedalboard.name.contains(&self.search_term) {
                            PedalboardListScreen::pedalboard_row(ui, pedalboard, row_size).0.map(|row_action| {
                                action = Some((i, row_action));
                            });
                            ui.end_row();
                        }
                    }
            }).response;

            if let Some((pedalboard_index, action)) = action {
                match action {
                    RowAction::Load => {
                        let pedalboard = pedalboard_library.get(pedalboard_index).unwrap();
                        self.state.active_pedalboardset.borrow_mut().pedalboards.push(pedalboard.clone());
                    },
                    RowAction::Delete => {
                        drop(pedalboard_library);
                        let mut pedalboard_library = self.state.pedalboard_library.borrow_mut();
                        pedalboard_library.remove(pedalboard_index);
                    }
                }
            };

            response
        }
    }
}