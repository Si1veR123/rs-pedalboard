mod socket;
mod state;
use state::State;

mod pedalboard_set_screen;
use pedalboard_set_screen::PedalboardSetScreen;
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

fn main() {
    //let mut socket = ClientSocket::new(29475);
    //socket.connect().expect("Failed to connect to server");

    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = native_options.viewport.with_inner_size((WINDOW_WIDTH, WINDOW_HEIGHT));
    native_options.viewport.resizable = Some(false);

    eframe::run_native("Pedalboard", native_options, Box::new(
        |cc| Ok(Box::new(PedalboardClientApp::new(cc)))
    )).expect("Failed to run app");
}




pub struct PedalboardClientApp {
    //socket: ClientSocket,

    state: &'static State,

    selected_screen: usize,
    pedalboard_set_screen: PedalboardSetScreen,
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
            pedalboard_set_screen: PedalboardSetScreen::new(state),
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
                    columns[0].horizontal_centered(|ui| {
                        if ui.add_sized(button_size, egui::Button::new(
                            RichText::new("Set View").size(20.0)
                        )).clicked() {
                            self.selected_screen = 0;
                        }
                    });
                    columns[1].horizontal_centered(|ui| {
                        if ui.add_sized(button_size, egui::Button::new(
                            RichText::new("Library").size(20.0)
                        )).clicked() {
                            self.selected_screen = 1;
                        }
                    });
                    columns[2].horizontal_centered(|ui| {
                        if ui.add_sized(button_size, egui::Button::new(
                            RichText::new("Songs").size(20.0)
                        )).clicked() {
                            self.selected_screen = 2;
                        }
                    });
                    columns[3].horizontal_centered(|ui| {
                        if ui.add_sized(button_size, egui::Button::new(
                            RichText::new("Utilities").size(20.0)
                        )).clicked() {
                            self.selected_screen = 3;
                        }
                    });
                });
        });

        egui::CentralPanel::default().show(&ctx, |ui| {
            match self.selected_screen {
                0 => {
                    ui.add(&mut self.pedalboard_set_screen);
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
