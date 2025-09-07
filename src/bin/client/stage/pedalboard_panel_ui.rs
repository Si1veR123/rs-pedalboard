use super::{CurrentAction, PedalboardStageScreen};

use eframe::egui::{self, Color32, Layout, Rgba, RichText, Vec2};
use egui_dnd::dnd;
use rs_pedalboard::pedalboard_set::PedalboardSet;
use crate::THEME_COLOUR;

// Big ugly function to display the pedalboard stage panel
// Effectively a method on PedalboardStageScreen
pub fn pedalboard_stage_panel(screen: &mut PedalboardStageScreen, ui: &mut egui::Ui) {
    ui.painter().rect_filled(ui.available_rect_before_wrap(), 5.0, crate::LIGHT_BACKGROUND_COLOR);

    let mut pedalboard_library = screen.state.pedalboards.pedalboard_library.borrow_mut();

    ui.add_space(5.0);

    // === Header buttons ===
    let buttons_row_height = ui.available_height() * 0.075;
    ui.columns(2, |columns| {
        columns[0].allocate_ui_with_layout(
            Vec2::new(0.0, buttons_row_height),
            Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(10.0);
                if ui.add_sized([100.0, ui.available_height()], egui::Button::new("Clear Stage")).clicked() {
                    *screen.state.pedalboards.active_pedalboardstage.borrow_mut() = PedalboardSet::default();
                    screen.state.load_active_set();
                }
            }
        );

        columns[1].allocate_ui_with_layout(
            Vec2::new(0.0, buttons_row_height),
            Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.add_space(10.0);
                if ui.add_sized([100.0, ui.available_height()], egui::Button::new("Save to Song")).clicked() {
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

    egui::ScrollArea::vertical().show(ui, |ui| {
        let active_pedalboards = screen.state.pedalboards.active_pedalboardstage.borrow_mut();

        let dnd_response = dnd(ui, "pedalboard_dnd").show_sized(
            active_pedalboards.pedalboards.iter().enumerate(),
            Vec2::new(row_width, row_height),
            |ui, (i, pedalboard), handle, _state| {
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
                                Color32::from(THEME_COLOUR.linear_multiply(0.2))
                            );
                        } else if i % 2 == 0 {
                            ui.painter().rect_filled(
                                ui.available_rect_before_wrap(),
                                5.0,
                                crate::LIGHT_BACKGROUND_COLOR.gamma_multiply(1.2)
                            );
                        }
                        
                        ui.columns(2, |columns| {
                            columns[0].horizontal_centered(|ui| {
                                let (text_color, drag_icon_color) = if active_pedalboards.active_pedalboard == i {
                                    (Rgba::from_white_alpha(0.9), Color32::from_gray(80))
                                } else {
                                    (Rgba::from_white_alpha(0.7), Color32::from_gray(50))
                                };

                                handle.ui(ui, |ui| {
                                    ui.add_space(15.0);
                                    ui.add(
                                        egui::Image::new(egui::include_image!("../files/drag.png"))
                                        .tint(drag_icon_color)
                                        .max_width(15.0)
                                    );
                                    ui.add_space(2.0);
                                });

                                if ui.label(RichText::new(pedalboard.name.clone()).color(text_color)
                                    .size(20.0)
                                    .font(
                                        crate::default_proportional(20.0)
                                    )).clicked()
                                {
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
                                            screen.current_action = Some(CurrentAction::DuplicateLinked(i));
                                        }
                                        ui.add_space(2.0);
                                        ui.separator();
                                        ui.add_space(2.0);
                                        if ui.add(egui::Button::new(RichText::new("Duplicate New").size(25.0))).clicked() {
                                            screen.current_action = Some(CurrentAction::DuplicateNew(i));
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


                let active_index = active_pedalboards.active_pedalboard;
                drop(active_pedalboards);
                if drag_update.from == active_index {
                    screen.state.play(new_pedalboard_index, false);
                }
                else if drag_update.from < active_index && drag_update.to > active_index {
                    screen.state.play(active_index-1, false);
                }
                else if drag_update.from > active_index && drag_update.to <= active_index {
                    screen.state.play(active_index+1, false);
                }

                screen.state.move_pedalboard(drag_update.from, new_pedalboard_index, false);
            }
        }
    }).inner;

    // === Perform actions ===
    let active_pedalboards = screen.state.pedalboards.active_pedalboardstage.borrow_mut();
    match screen.current_action.take() {
        Some(CurrentAction::DuplicateLinked(index)) => {
            drop(active_pedalboards);
            screen.state.duplicate_linked(index);
        },
        Some(CurrentAction::DuplicateNew(index)) => {
            drop(active_pedalboards);
            screen.state.duplicate_new(index);
        },
        Some(CurrentAction::Remove(index)) => {
            drop(active_pedalboards);
            screen.state.remove_pedalboard_from_stage(index, false);
        },
        Some(CurrentAction::SaveToSong(mut song_name)) => {
            let mut open = true;
            drop(active_pedalboards);
            let saved= screen.save_song_input_window(ui, "Save to Song", &mut song_name, &mut open);

            if saved {
                drop(pedalboard_library);
                screen.state.pedalboards.save_to_song(song_name.clone());
            } else if open {
                screen.current_action = Some(CurrentAction::SaveToSong(song_name));
            }
        },
        Some(CurrentAction::Rename((index, mut new_name))) => {
            let mut open = true;
            let saved = screen.input_string_window(ui, "Rename", &mut new_name, &mut open);

            if saved {
                let pedalboard_id = active_pedalboards.pedalboards.get(index).unwrap().get_id();
                drop(active_pedalboards);
                drop(pedalboard_library);
                screen.state.rename_pedalboard(pedalboard_id, new_name);
            } else if open {
                screen.current_action = Some(CurrentAction::Rename((index, new_name)));
            }
        },
        Some(CurrentAction::SaveToLibrary(index)) => {
            let pedalboard = active_pedalboards.pedalboards.get(index).unwrap().clone();
            pedalboard_library.push(pedalboard);
        },
        Some(CurrentAction::ChangeActive(index)) => {
            drop(active_pedalboards);
            screen.state.play(index, false);
        },
        None => {}
    }
}