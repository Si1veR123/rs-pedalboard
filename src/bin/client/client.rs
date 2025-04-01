mod socket;
mod state;
use state::State;

mod pedalboard_stage_screen;
use pedalboard_stage_screen::PedalboardstageScreen;
mod pedalboard_library_screen;
use pedalboard_library_screen::PedalboardLibraryScreen;
mod songs_screen;
use songs_screen::SongsScreen;
mod utilities_screen;
use utilities_screen::UtilitiesScreen;

use eframe::egui::{self, Id, RichText};

const SERVER_PORT: u16 = 29475;
const WINDOW_HEIGHT: f32 = 600.0;
const WINDOW_WIDTH: f32 = 1024.0;

pub const THEME_COLOUR: egui::Color32 = egui::Color32::from_rgb(255, 105, 46);
pub const ROW_COLOUR_LIGHT: egui::Color32 = egui::Color32::from_gray(26);
pub const ROW_COLOUR_DARK: egui::Color32 = egui::Color32::from_gray(21);
pub const TEXT_COLOUR: egui::Color32 = egui::Color32::from_gray(200);
pub const EXTREME_BACKGROUND_COLOUR: egui::Color32 = egui::Color32::from_gray(2);
pub const BACKGROUND_COLOUR: egui::Color32 = egui::Color32::from_gray(15);
pub const WIDGET_BACKGROUND_COLOUR: egui::Color32 = egui::Color32::from_gray(34);
pub const WIDGET_HOVER_BACKGROUND_COLOUR: egui::Color32 = egui::Color32::from_gray(40);
pub const WIDGET_CLICK_BACKGROUND_COLOUR_THEME_ALPHA: f32 = 0.025;

fn main() {
    //let mut socket = ClientSocket::new(29475);
    //socket.connect().expect("Failed to connect to server");

    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = native_options.viewport.with_inner_size((WINDOW_WIDTH, WINDOW_HEIGHT));
    native_options.viewport.resizable = Some(false);

    eframe::run_native("Pedalboard", native_options, Box::new(
        |cc| {
            cc.egui_ctx.style_mut(|style| {
                style.visuals.extreme_bg_color = EXTREME_BACKGROUND_COLOUR.into();
                style.visuals.panel_fill = BACKGROUND_COLOUR.into();
                style.visuals.override_text_color = Some(TEXT_COLOUR.into());
                style.visuals.extreme_bg_color = EXTREME_BACKGROUND_COLOUR.into();
                let widget_click_background_color = THEME_COLOUR.linear_multiply(WIDGET_CLICK_BACKGROUND_COLOUR_THEME_ALPHA);
                style.visuals.widgets.active.bg_fill = widget_click_background_color.into();
                style.visuals.widgets.active.weak_bg_fill = widget_click_background_color.into();
                style.visuals.widgets.hovered.bg_fill = WIDGET_HOVER_BACKGROUND_COLOUR.into();
                style.visuals.widgets.hovered.weak_bg_fill = WIDGET_HOVER_BACKGROUND_COLOUR.into();
                style.visuals.widgets.inactive.bg_fill = WIDGET_BACKGROUND_COLOUR.into();
                style.visuals.widgets.inactive.weak_bg_fill = WIDGET_BACKGROUND_COLOUR.into();
                style.visuals.widgets.active.bg_stroke = (1.0, THEME_COLOUR).into();
            });
            Ok(Box::new(PedalboardClientApp::new(cc)))
        }
    )).expect("Failed to run app");
}




pub struct PedalboardClientApp {
    //socket: ClientSocket,

    state: &'static State,

    selected_screen: usize,
    pedalboard_stage_screen: PedalboardstageScreen,
    pedalboard_library_screen: PedalboardLibraryScreen,
    songs_screen: SongsScreen,
    utilities_screen: UtilitiesScreen,
}

impl PedalboardClientApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        //let mut socket = ClientSocket::new(SERVER_PORT);
        //socket.connect().expect("Failed to connect to server");
        let state = Box::leak(Box::new(State::default()));

        PedalboardClientApp {
            //socket,
            state,

            selected_screen: 0,
            pedalboard_stage_screen: PedalboardstageScreen::new(state),
            pedalboard_library_screen: PedalboardLibraryScreen::new(state),
            songs_screen: SongsScreen::new(state),
            utilities_screen: UtilitiesScreen::new(),
        }
    }
}

impl eframe::App for PedalboardClientApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom(Id::new("bottom_window_select"))
            .min_height(WINDOW_HEIGHT / 10.0)
            .show(&ctx, |ui| {
                ui.columns(4, |columns| {
                    let button_size = [columns[0].available_width(), columns[0].available_height() - 10.0];

                    let mut button_outline = [egui::Stroke::new(0.3, egui::Color32::BLACK); 4];
                    button_outline[self.selected_screen] = egui::Stroke::new(1.0, THEME_COLOUR);
                    let mut button_bg = [egui::Color32::from_gray(23); 4];
                    button_bg[self.selected_screen] = egui::Color32::from_gray(33);

                    columns[0].horizontal_centered(|ui| {
                        if ui.add_sized(button_size, egui::Button::new(
                            RichText::new("Stage View").size(20.0)
                        ).stroke(button_outline[0]).fill(button_bg[0])).clicked() {
                            self.selected_screen = 0;
                        }
                    });
                    columns[1].horizontal_centered(|ui| {
                        if ui.add_sized(button_size, egui::Button::new(
                            RichText::new("Library").size(20.0)
                        ).stroke(button_outline[1]).fill(button_bg[1])).clicked() {
                            self.selected_screen = 1;
                        }
                    });
                    columns[2].horizontal_centered(|ui| {
                        if ui.add_sized(button_size, egui::Button::new(
                            RichText::new("Songs").size(20.0)
                        ).stroke(button_outline[2]).fill(button_bg[2])).clicked() {
                            self.selected_screen = 2;
                        }
                    });
                    columns[3].horizontal_centered(|ui| {
                        if ui.add_sized(button_size, egui::Button::new(
                            RichText::new("Utilities").size(20.0)
                        ).stroke(button_outline[3]).fill(button_bg[3])).clicked() {
                            self.selected_screen = 3;
                        }
                    });
                });
        });

        egui::CentralPanel::default().show(&ctx, |ui| {
            match self.selected_screen {
                0 => {
                    ui.add(&mut self.pedalboard_stage_screen);
                },
                1 => {
                    ui.add(&mut self.pedalboard_library_screen);
                },
                2 => {
                    ui.add(&mut self.songs_screen);
                },
                3 => {
                    ui.add(&mut self.utilities_screen);
                },
                _ => {
                    ui.label("Invalid screen selected");
                }
            }
        });
    }
}
