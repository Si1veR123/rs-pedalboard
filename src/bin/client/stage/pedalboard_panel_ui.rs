use super::{CurrentAction, PedalboardStageScreen};

use eframe::egui::{self, Color32, Layout, Rgba, RichText, Vec2};
use rs_pedalboard::pedalboard_set::PedalboardSet;
use crate::THEME_COLOUR;

struct PedalboardDragPayload {
    pedalboard_origin_index: usize
} 

// Big ugly function to display the pedalboard stage panel
// Effectively a method on PedalboardStageScreen
pub fn pedalboard_stage_panel(screen: &mut PedalboardStageScreen, ui: &mut egui::Ui) -> egui::Response {
    let mut active_pedalboards = screen.state.active_pedalboardstage.borrow_mut();
    let mut pedalboard_library = screen.state.pedalboard_library.borrow_mut();

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
                    screen.socket.borrow_mut().load_set(&active_pedalboards);
                }
            }
        );

        columns[1].allocate_ui_with_layout(
            Vec2::new(0.0, buttons_row_height),
            Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.add_space(10.0);
                if ui.add_sized([100.0, buttons_row_height], egui::Button::new("Save to Song")).clicked() {
                    screen.current_action = Some(CurrentAction::SaveToSong(String::new()));
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
                                screen.current_action = Some(CurrentAction::ChangeActive(i));
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
                                    let (text_color, drag_icon_color) = if active_pedalboards.active_pedalboard == i {
                                        (Rgba::from_white_alpha(0.9), Color32::from_gray(15).linear_multiply(0.7))
                                    } else {
                                        (Rgba::from_white_alpha(0.4), Color32::from_gray(50).linear_multiply(0.7))
                                    };

                                    let drag_response = ui.allocate_ui_with_layout(
                                        Vec2::new(row_width, row_height),
                                        Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            ui.add_space(15.0);
                                            ui.add(
                                                egui::Image::new(egui::include_image!("../images/drag.png"))
                                                .tint(drag_icon_color)
                                                .max_width(15.0)
                                            );
                                            ui.add_space(2.0);
                                        }
                                    ).response;
                                    if drag_response.dragged() {
                                        let payload = PedalboardDragPayload { pedalboard_origin_index: i };
                                        dbg!(i);
                                    }

                                    if ui.label(RichText::new(pedalboard.name.clone()).color(text_color).size(20.0)).clicked() {
                                        screen.current_action = Some(CurrentAction::ChangeActive(i));
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
                                                screen.current_action = Some(CurrentAction::Remove(i));
                                            }
                                            ui.add_space(2.0);
                                            ui.separator();
                                            ui.add_space(2.0);
                                            if ui.add(egui::Button::new(RichText::new("Rename").size(25.0))).clicked() {
                                                screen.current_action = Some(CurrentAction::Rename((i, pedalboard.name.clone())));
                                            }
                                            ui.add_space(2.0);
                                            ui.separator();
                                            ui.add_space(2.0);
                                            if ui.add(egui::Button::new(RichText::new("Duplicate").size(25.0))).clicked() {
                                                screen.current_action = Some(CurrentAction::Duplicate(i));
                                            }
                                            ui.add_space(5.0);
                                        });

                                        ui.add_space(5.0);

                                        let in_library = pedalboard_library.iter().any(|library_pedalboard| library_pedalboard.name == pedalboard.name);
                                        if in_library {
                                            ui.add(egui::Button::new("Saved"));
                                        } else {
                                            if ui.add(egui::Button::new("Save")).clicked() {
                                                screen.current_action = Some(CurrentAction::SaveToLibrary(i));
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
    match screen.current_action.take() {
        Some(CurrentAction::Duplicate(index)) => {
            let mut pedalboard = active_pedalboards.pedalboards.get(index).unwrap().clone();
            let pedalboardset_length = active_pedalboards.pedalboards.len();
            drop(active_pedalboards);
            drop(pedalboard_library);
            let unique_name = screen.state.unique_pedalboard_name(pedalboard.name.clone());
            pedalboard.name = unique_name;

            let mut socket = screen.socket.borrow_mut();
            socket.add_pedalboard(&pedalboard).expect("Failed to add pedalboard");
            socket.move_pedalboard(pedalboardset_length-1, index+1).expect("Failed to move pedalboard");

            screen.state.active_pedalboardstage.borrow_mut().pedalboards.insert(index+1, pedalboard);
        },
        Some(CurrentAction::Remove(index)) => {
            if active_pedalboards.remove_pedalboard(index) {
                let mut socket = screen.socket.borrow_mut();
                socket.delete_pedalboard(index).expect("Failed to remove pedalboard");
            }
        },
        Some(CurrentAction::SaveToSong(mut song_name)) => {
            let mut open = true;
            drop(active_pedalboards);
            let saved = screen.input_string_window(ui, "Save to Song", &mut song_name, &mut open);

            if saved {
                let active_pedalboards = &screen.state.active_pedalboardstage.borrow().pedalboards;
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
                screen.state.songs_library.borrow_mut().insert(song_name.clone(), pedalboard_names);
            } else if open {
                screen.current_action = Some(CurrentAction::SaveToSong(song_name));
            }
        },
        Some(CurrentAction::Rename((index, mut new_name))) => {
            let mut open = true;
            let saved = screen.input_string_window(ui, "Rename", &mut new_name, &mut open);

            if saved {
                let old_name = active_pedalboards.pedalboards.get(index).unwrap().name.clone();
                drop(active_pedalboards);
                drop(pedalboard_library);
                let unique_name = screen.state.unique_pedalboard_name(new_name.clone());
                screen.state.rename_pedalboard(&old_name, &unique_name);
            }

            if open {
                screen.current_action = Some(CurrentAction::Rename((index, new_name)));
            }
        },
        Some(CurrentAction::SaveToLibrary(index)) => {
            let pedalboard = active_pedalboards.pedalboards.get(index).unwrap().clone();
            pedalboard_library.push(pedalboard);
        },
        Some(CurrentAction::ChangeActive(index)) => {
            active_pedalboards.active_pedalboard = index;

            let mut socket = screen.socket.borrow_mut();
            socket.play(index).expect("Failed to change active pedalboard");
        },
        None => {}
    }

    response
}