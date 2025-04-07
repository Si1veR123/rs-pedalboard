use std::hash::Hash;

use super::{CurrentAction, PedalboardStageScreen};

use eframe::egui::{self, Color32, Id, Layout, Rgba, RichText, Vec2};
use egui_dnd::{dnd, DragDropItem};
use rs_pedalboard::{pedalboard::{self, Pedalboard}, pedalboard_set::PedalboardSet};
use crate::THEME_COLOUR;

// Big ugly function to display the pedalboard stage panel
// Effectively a method on PedalboardStageScreen
pub fn pedalboard_stage_panel(screen: &mut PedalboardStageScreen, ui: &mut egui::Ui) {
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
                    screen.current_action = Some(CurrentAction::SaveToSong((String::new(), false)));
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

    egui::ScrollArea::vertical().show(ui, |ui| {
        let dnd_response = dnd(ui, "pedalboard_dnd").show_sized(
            active_pedalboards.pedalboards.iter().enumerate(),
            Vec2::new(row_width, row_height),
            |ui, (i, pedalboard), handle, state| {
                ui.allocate_ui_with_layout(
                    Vec2::new(row_width, row_height),
                    Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        // === Each Row ===
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

                                handle.ui(ui, |ui| {
                                    ui.add_space(15.0);
                                    ui.add(
                                        egui::Image::new(egui::include_image!("../images/drag.png"))
                                        .tint(drag_icon_color)
                                        .max_width(15.0)
                                    );
                                    ui.add_space(2.0);
                                });

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

                                    if ui.add(egui::Button::new("Save")).clicked() {
                                        screen.current_action = Some(CurrentAction::SaveToLibrary((i, false)));
                                    }
                                }
                            )
                        }
                    )
                });
                ui.end_row();
            }
        );

        if let Some(drag_update) = dnd_response.final_update() {
            let pedalboard_count = active_pedalboards.pedalboards.len();

            if drag_update.to <= pedalboard_count &&
               drag_update.from <= pedalboard_count &&
               drag_update.from != drag_update.to
            {
                let moving_down = drag_update.from < drag_update.to;

                // If moving down, the new index of the pedalboard is the one before the one we are moving to
                let new_pedalboard_index = if moving_down {
                    drag_update.to - 1
                } else {
                    drag_update.to
                };

                let mut socket = screen.socket.borrow_mut();
                let active_index = active_pedalboards.active_pedalboard;

                if drag_update.from == active_index {
                    active_pedalboards.active_pedalboard = new_pedalboard_index;
                    socket.play(active_pedalboards.active_pedalboard);
                }
                else if drag_update.from < active_index && drag_update.to > active_index {
                    active_pedalboards.active_pedalboard -= 1;
                    socket.play(active_pedalboards.active_pedalboard);
                }
                else if drag_update.from > active_index && drag_update.to <= active_index {
                    active_pedalboards.active_pedalboard += 1;
                    socket.play(active_pedalboards.active_pedalboard);
                }

                socket.move_pedalboard(drag_update.from, new_pedalboard_index).expect("Socket failed to move pedalboard");
                dnd_response.update_vec(&mut active_pedalboards.pedalboards);
            }
        }
    }).inner;

    // === Perform actions ===
    match screen.current_action.take() {
        Some(CurrentAction::Duplicate(index)) => {
            let mut pedalboard = active_pedalboards.pedalboards.get(index).unwrap().clone();
            let pedalboardset_length = active_pedalboards.pedalboards.len();
            drop(active_pedalboards);
            drop(pedalboard_library);
            let unique_name = screen.state.unique_stage_pedalboard_name(pedalboard.name.clone());
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
        Some(CurrentAction::SaveToSong((mut song_name, mut checked))) => {
            let mut open = true;
            drop(active_pedalboards);
            let saved= screen.save_song_input_window(ui, "Save to Song", &mut song_name, &mut checked, &mut open);

            if saved {
                let active_pedalboards = &screen.state.active_pedalboardstage.borrow().pedalboards;
                
                // The actual names of the pedalboards that are saved in the song, as some may be renamed
                // due to them existing in the library already
                let mut actual_song_pedalboard_names = Vec::with_capacity(active_pedalboards.len());

                drop(pedalboard_library);

                for pedalboard in active_pedalboards.iter() {
                    let mut pedalboard_library = screen.state.pedalboard_library.borrow_mut();
                    let pedalboard_in_library = pedalboard_library.iter_mut().find(|library_pedalboard| library_pedalboard.name == pedalboard.name);
                    if pedalboard_in_library.is_some() {
                        if checked {
                            // Overwrite existing pedalboard in library
                            *pedalboard_in_library.unwrap() = pedalboard.clone();
                            actual_song_pedalboard_names.push(pedalboard.name.clone());
                        } else {
                            // Unique library name function needs to borrow the library
                            // so we need to drop the mutable borrow first
                            drop(pedalboard_library);
                            // Create pedalboard with new unique name in library
                            let new_pedalboard_name = screen.state.unique_library_pedalboard_name(pedalboard.name.clone());

                            let mut pedalboard = pedalboard.clone();
                            pedalboard.name = new_pedalboard_name.clone();
                            screen.state.pedalboard_library.borrow_mut().push(pedalboard);
                            actual_song_pedalboard_names.push(new_pedalboard_name);
                        }
                    } else {
                        // Save pedalboard to library with existing name
                        pedalboard_library.push(pedalboard.clone());
                        actual_song_pedalboard_names.push(pedalboard.name.clone());
                    }
                }

                screen.state.songs_library.borrow_mut().insert(song_name.clone(), actual_song_pedalboard_names);
            } else if open {
                screen.current_action = Some(CurrentAction::SaveToSong((song_name, checked)));
            }
        },
        Some(CurrentAction::Rename((index, mut new_name))) => {
            let mut open = true;
            let saved = screen.input_string_window(ui, "Rename", &mut new_name, &mut open);

            if saved {
                let old_name = active_pedalboards.pedalboards.get(index).unwrap().name.clone();
                drop(active_pedalboards);
                drop(pedalboard_library);
                let unique_name = screen.state.unique_stage_pedalboard_name(new_name.clone());
                screen.state.rename_stage_pedalboard(&old_name, &unique_name);
            }

            if open {
                screen.current_action = Some(CurrentAction::Rename((index, new_name)));
            }
        },
        Some(CurrentAction::SaveToLibrary((index, mut window_open))) => {
            let pedalboard = active_pedalboards.pedalboards.get(index).unwrap();

            if window_open {
                let selected = screen.save_overwrite_window(ui, &mut window_open);
                if let Some(overwrite) = selected {
                    if overwrite {
                        // Overwrite existing pedalboard in library
                        let pedalboard_in_library = pedalboard_library.iter_mut().find(|library_pedalboard| library_pedalboard.name == pedalboard.name).unwrap();
                        *pedalboard_in_library = pedalboard.clone();
                    } else {
                        // Create pedalboard with new unique name in library

                        drop(pedalboard_library);
                        let new_pedalboard_name = screen.state.unique_library_pedalboard_name(pedalboard.name.clone());
                        let mut pedalboard = pedalboard.clone();
                        pedalboard.name = new_pedalboard_name;
                        screen.state.pedalboard_library.borrow_mut().push(pedalboard);
                    }
                } else {
                    screen.current_action = Some(CurrentAction::SaveToLibrary((index, window_open)));
                }
            } else {
                let pedalboard_exists = pedalboard_library.iter().any(|library_pedalboard| library_pedalboard.name == pedalboard.name);
                if pedalboard_exists {
                    screen.current_action = Some(CurrentAction::SaveToLibrary((index, true)));
                } else {
                    pedalboard_library.push(pedalboard.clone());
                }
            }
        },
        Some(CurrentAction::ChangeActive(index)) => {
            active_pedalboards.active_pedalboard = index;

            let mut socket = screen.socket.borrow_mut();
            socket.play(index).expect("Failed to change active pedalboard");
        },
        None => {}
    }
}