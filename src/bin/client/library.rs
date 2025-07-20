use eframe::egui::{self, Layout, RichText, TextEdit, Vec2, Widget};
use rs_pedalboard::pedalboard::Pedalboard;
use crate::state::State;

pub enum RowAction {
    Load,
    Delete
}

pub struct PedalboardLibraryScreen {
    // Store pedalboards by unique name
    state: &'static State,
    search_term: String,
}

impl PedalboardLibraryScreen {
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
                            if ui.add_sized(
                                [80.0, 40.0],
                                egui::Button::new("Delete").stroke((1.5, egui::Color32::from_rgb(150, 30, 30)))
                            ).clicked() {
                                action = Some(RowAction::Delete);
                            }
                            if ui.add_sized(
                                [80.0, 40.0],
                                egui::Button::new("Load").stroke((1.3, egui::Color32::from_gray(60)))
                            ).clicked() {
                                action = Some(RowAction::Load);
                            }
                        }
                    )
                });
        }).response;

        (action, response)
    }
}

impl Widget for &mut PedalboardLibraryScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.add_space(5.0);

        // === Search bar and new pedalboard button ===
        ui.columns(3, |columns| {
            columns[1]
                .add_sized(
                    [0.0, 40.0],
                    TextEdit::singleline(&mut self.search_term).hint_text(RichText::new("Search pedalboards...").size(20.0))
                );


            columns[0].allocate_ui_with_layout(
                Vec2::new(0.0, 40.0),
                Layout::top_down(egui::Align::Center),
                |ui| {
                    if ui.add_sized(
                        [200.0, 40.0],
                        egui::Button::new(
                            RichText::new("New Pedalboard").size(20.0)
                        ).stroke((0.7, crate::THEME_COLOUR))).clicked()
                    {
                        let unique_name = self.state.pedalboards.unique_library_pedalboard_name(String::from("New Pedalboard"));
                        self.state.pedalboards.pedalboard_library.borrow_mut().push(Pedalboard::new(unique_name));
                }
            });
        });

        ui.add_space(5.0);
        ui.separator();
        ui.add_space(10.0);

        // === Pedalboard Grid ===
        let row_height = 50.0;
        let row_size = Vec2::new(ui.available_width(), row_height);

        let pedalboard_library = self.state.pedalboards.pedalboard_library.borrow();
        if pedalboard_library.is_empty() {
            ui.add_sized(row_size, egui::Label::new(RichText::new("No Pedalboards Found").size(30.0)))
        } else {
            let mut action = None;

            egui::ScrollArea::vertical().show(ui, |ui| {
                let response = egui::Grid::new("pedalboard_library_grid")
                    .with_row_color(|index, _style| {
                        if index % 2 == 0 {
                            Some(crate::ROW_COLOUR_LIGHT)
                        } else {
                            Some(crate::ROW_COLOUR_DARK)
                        }
                    })
                    .spacing(Vec2::new(10.0, 20.0))
                    .show(ui, |ui| {
                        for (i, pedalboard) in pedalboard_library.iter().enumerate() {
                            if self.search_term.is_empty() || pedalboard.name.contains(&self.search_term) {
                                PedalboardLibraryScreen::pedalboard_row(ui, pedalboard, row_size).0.map(|row_action| {
                                    action = Some((i, row_action));
                                });
                                ui.end_row();
                            }
                        }
                }).response;

                // Perform any actions performed in this frame
                if let Some((pedalboard_index, action)) = action {
                    match action {
                        RowAction::Load => {
                            let pedalboard = pedalboard_library.get(pedalboard_index).unwrap();
                            self.state.add_pedalboard(pedalboard.clone());
                        },
                        RowAction::Delete => {
                            let pedalboard_name = &pedalboard_library.get(pedalboard_index).unwrap().name.clone();
                            drop(pedalboard_library);
                            self.state.pedalboards.delete_pedalboard(&pedalboard_name);
                        }
                    }
                };

                response
            }).inner
        }
    }
}