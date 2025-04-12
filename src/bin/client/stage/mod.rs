mod pedalboard_panel_ui;
use pedalboard_panel_ui::pedalboard_stage_panel;

mod pedalboard_designer;
use pedalboard_designer::pedalboard_designer;

use eframe::egui::{self, Layout, Vec2, Widget};
use rs_pedalboard::pedals::{Pedal, PedalDiscriminants};
use strum::IntoEnumIterator;
use crate::state::State;

pub enum CurrentAction {
    DuplicateLinked(usize),
    Remove(usize),
    SaveToSong(String),
    Rename((usize, String)),
    SaveToLibrary(usize),
    ChangeActive(usize)
}

pub struct PedalboardStageScreen {
    state: &'static State,
    show_pedal_menu: bool,
    current_action: Option<CurrentAction>,
}

impl PedalboardStageScreen {
    pub fn new(state: &'static State) -> Self {
        Self {
            state,
            show_pedal_menu: false,
            current_action: None
        }
    }

    fn save_song_input_window(&mut self, ui: &mut egui::Ui, title: &str, input: &mut String, open: &mut bool) -> bool {
        let mut saved = false;
        egui::Window::new(title)
            .open(open)
            .show(ui.ctx(), |ui| {
                ui.add(egui::TextEdit::singleline(input));
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
