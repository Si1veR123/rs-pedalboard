mod pedalboard_panel_ui;
use pedalboard_panel_ui::pedalboard_stage_panel;

mod pedalboard_designer;
use pedalboard_designer::pedalboard_designer;

use std::cell::RefCell;
use std::rc::Rc;

use eframe::egui::{self, Layout, Vec2, Widget};
use crate::socket::ClientSocket;
use crate::state::State;

pub enum CurrentAction {
    Duplicate(usize),
    Remove(usize),
    SaveToSong((String, bool)),
    Rename((usize, String)),
    SaveToLibrary((usize, bool)),
    ChangeActive(usize)
}

pub struct PedalboardStageScreen {
    state: &'static State,
    pub socket: Rc<RefCell<ClientSocket>>,
    current_action: Option<CurrentAction>,
}

impl PedalboardStageScreen {
    pub fn new(state: &'static State, socket: Rc<RefCell<ClientSocket>>) -> Self {
        Self {
            state,
            socket,
            current_action: None
        }
    }

    fn save_song_input_window(&mut self, ui: &mut egui::Ui, title: &str, input: &mut String, checked: &mut bool, open: &mut bool) -> bool {
        let mut saved = false;
        egui::Window::new(title)
            .open(open)
            .show(ui.ctx(), |ui| {
                ui.add(egui::TextEdit::singleline(input));
                ui.add(egui::Checkbox::new(checked, "Overwrite existing pedalboards in library(s)"));
                if ui.button("Save Song").clicked() {
                    saved = true;
                }
            });

        if saved {
            *open = false;
        }

        saved
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

    // Returns yes or no, or None if cancelled/not selected this frame
    fn save_overwrite_window(&mut self, ui: &mut egui::Ui, open: &mut bool) -> Option<bool> {
        let mut saved = None;
        egui::Window::new("Overwrite")
            .show(ui.ctx(), |ui| {
                ui.label("Overwrite existing pedalboard(s) or save with unique name?");
                if ui.button("Overwrite").clicked() {
                    *open = false;
                    saved = Some(true);
                }
                if ui.button("Unique Name").clicked() {
                    *open = true;
                    saved = Some(false);
                }
            });

        saved
    }
}

impl Widget for &mut PedalboardStageScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let width = ui.available_width();
        let height = ui.available_height();
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(width * 0.4, height),
                    Layout::top_down(egui::Align::Center),
                    |ui| pedalboard_stage_panel(self, ui)
            );
            ui.allocate_ui_with_layout(
                Vec2::new(width * 0.6, height),
                Layout::top_down(egui::Align::Center),
                |ui| pedalboard_designer(self, ui)
            );
        }).response
    }
}
