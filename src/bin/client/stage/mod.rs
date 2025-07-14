mod pedalboard_panel_ui;
use std::time::{Duration, Instant};

use pedalboard_panel_ui::pedalboard_stage_panel;

mod pedalboard_designer;
use pedalboard_designer::pedalboard_designer;

use eframe::egui::{self, Layout, Rect, Vec2, Widget};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use crate::state::State;

/// How often should the stage screen be refreshed when not interacted.
/// This is to update stats, time, etc.
const STAGE_REPAINT_DURATION: std::time::Duration = Duration::from_secs(1);

pub enum CurrentAction {
    DuplicateLinked(usize),
    DuplicateNew(usize),
    Remove(usize),
    SaveToSong(String),
    Rename((usize, String)),
    SaveToLibrary(usize),
    ChangeActive(usize)
}

pub enum ClippingState {
    None,
    // Time that the last clip occurred
    Clipping(Instant)
}

pub enum XRunState {
    None,
    // How many occurred since the first one, time that the last xrun occurred
    Few((usize, Instant)),
    // Time that the last xrun occurred
    Many(Instant)
}

pub struct PedalboardStageScreen {
    state: &'static State,
    show_pedal_menu: bool,
    current_action: Option<CurrentAction>,
    // For the Scene in pedalboard designer
    pedalboard_rect: Rect,
    // For CPU/RAM usage
    system: System,
    last_system_refresh: std::time::Instant,

    command_buffer: Vec<String>,
    xrun_state: XRunState,
    clipping_state: ClippingState
}

impl PedalboardStageScreen {
    pub fn new(state: &'static State) -> Self {
        let refresh_kind = RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::nothing().with_cpu_usage())
            .with_memory(MemoryRefreshKind::nothing().with_ram());
        let mut system = System::new_with_specifics(refresh_kind);
        system.refresh_cpu_usage();
        system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());

        Self {
            state,
            show_pedal_menu: false,
            current_action: None,
            pedalboard_rect: Rect::ZERO,
            system,
            last_system_refresh: Instant::now(),
            command_buffer: Vec::new(),
            xrun_state: XRunState::None,
            clipping_state: ClippingState::None
        }
    }

    pub fn update_xrun_from_commands(&mut self) {
        self.command_buffer.clear();
        self.state.get_commands("xrun", &mut self.command_buffer);
        let xrun_count = self.command_buffer.len();

        match self.xrun_state {
            XRunState::None => {
                if xrun_count > 0 {
                    self.xrun_state = XRunState::Few((xrun_count, Instant::now()));
                }
            },
            XRunState::Few((count, last_xrun)) => {
                // If no xrun occurred for more than 2 seconds, reset the state
                if xrun_count == 0 && last_xrun.elapsed().as_secs() > 2 {
                    self.xrun_state = XRunState::None;
                    return;
                }

                // If more than 10 xruns have occurred, switch to Many state
                let total = count + xrun_count;
                if total > 10 {
                    self.xrun_state = XRunState::Many(Instant::now());
                } else if xrun_count > 0 {
                    self.xrun_state = XRunState::Few((total, Instant::now()));
                }
            },
            XRunState::Many(last_xrun) => {
                // If no xrun occurred for more than 2 seconds, reset the state
                if xrun_count == 0 && last_xrun.elapsed().as_secs() > 2 {
                    self.xrun_state = XRunState::None;
                } else if xrun_count > 0 {
                    self.xrun_state = XRunState::Many(Instant::now());
                }
            }
        }
    }

    pub fn update_clipping_from_commands(&mut self) {
        self.command_buffer.clear();
        self.state.get_commands("clipped", &mut self.command_buffer);
        if self.command_buffer.is_empty() {
            if let ClippingState::Clipping(last_clipping) = self.clipping_state {
                if last_clipping.elapsed().as_millis() > 500 {
                    self.clipping_state = ClippingState::None;
                }
            }
        } else {
            self.clipping_state = ClippingState::Clipping(Instant::now());
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
        ui.ctx().request_repaint_after(STAGE_REPAINT_DURATION);
        
        if self.last_system_refresh.elapsed() > sysinfo::MINIMUM_CPU_UPDATE_INTERVAL {
            self.system.refresh_cpu_usage();
            self.system.refresh_memory_specifics(sysinfo::MemoryRefreshKind::nothing().with_ram());
            self.last_system_refresh = Instant::now();
        }

        self.update_xrun_from_commands();
        self.update_clipping_from_commands();

        let right_padding = 5.0;
        let width = ui.available_width() - right_padding;
        let height = ui.available_height();
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(width * 0.33, height),
                    Layout::top_down(egui::Align::Center),
                    |ui| pedalboard_stage_panel(self, ui)
            );
            ui.allocate_ui_with_layout(
                Vec2::new(width * 0.67, height),
                Layout::top_down(egui::Align::Center),
                |ui| pedalboard_designer(self, ui)
            );
        }).response
    }
}
