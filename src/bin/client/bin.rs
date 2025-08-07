mod socket;
mod state;
use state::State;
mod saved_pedalboards;
mod stage;
use stage::PedalboardStageScreen;
mod library;
use library::PedalboardLibraryScreen;
mod songs;
use songs::SongsScreen;
mod utilities;
use utilities::UtilitiesScreen;
mod settings;
use settings::{SettingsScreen, ServerLaunchState};
mod server_process;

use eframe::egui::{self, include_image, Button, Color32, FontId, Id, ImageButton, RichText, Vec2};
use rs_pedalboard::SAVE_DIR;
use std::{sync::Arc, time::Instant};
use simplelog::*;

const SERVER_PORT: u16 = 29475;
const WINDOW_HEIGHT: f32 = 600.0;
const WINDOW_WIDTH: f32 = 1024.0;

pub const THEME_COLOUR: egui::Color32 = egui::Color32::from_rgb(255, 105, 46);
pub const ROW_COLOUR_LIGHT: egui::Color32 = egui::Color32::from_gray(28);
pub const ROW_COLOUR_DARK: egui::Color32 = egui::Color32::from_gray(22);
pub const TEXT_COLOUR: egui::Color32 = egui::Color32::from_gray(200);
pub const EXTREME_BACKGROUND_COLOUR: egui::Color32 = egui::Color32::from_gray(2);
pub const BACKGROUND_COLOUR: egui::Color32 = egui::Color32::from_gray(15);
pub const LIGHT_BACKGROUND_COLOR: egui::Color32 = egui::Color32::from_gray(22);
pub const WIDGET_BACKGROUND_COLOUR: egui::Color32 = egui::Color32::from_gray(34);
pub const WIDGET_HOVER_BACKGROUND_COLOUR: egui::Color32 = egui::Color32::from_gray(40);
pub const WIDGET_CLICK_BACKGROUND_COLOUR_THEME_ALPHA: f32 = 0.025;
// Buttons
pub const INACTIVE_BG_STROKE_COLOR: egui::Color32 = egui::Color32::from_gray(54);

/// Get a FontId for the egui default proportional font
pub fn default_proportional(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name("default-proportional".into()))
}

fn setup_custom_fonts(ctx: &egui::Context) {
    // Start with the default fonts (we will be adding to them rather than replacing them).
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "pedalboard_font".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!("files/TangoSans.ttf"))),
    );

    // Put the default proporional font in another font family so it can be used
    if let Some(font) = fonts.families.get(&egui::FontFamily::Proportional).and_then(|f| f.get(0)) {
        fonts.families.insert(
            egui::FontFamily::Name("default-proportional".into()),
            vec![font.clone()],
        );
    }

    // Put my font first (highest priority) for proportional text:
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "pedalboard_font".to_owned());

    // Put my font as last fallback for monospace:
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("pedalboard_font".to_owned());

    // Tell egui to use these fonts:
    ctx.set_fonts(fonts);

    // Change text style sizes
    ctx.style_mut(|style| {
        style.text_styles = [
            (egui::TextStyle::Small, FontId::new(12.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Body, FontId::new(18.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Button, FontId::new(18.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Heading, FontId::new(30.0, egui::FontFamily::Proportional)),
            (egui::TextStyle::Monospace, FontId::new(14.0, egui::FontFamily::Monospace)),
        ]
        .into();
    });
}

fn main() {
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Info, Config::default(), std::fs::File::create("pedalboard-client.log").expect("Failed to create log file")),
        ]
    ).expect("Failed to start logging");
    log::info!("Started logging...");

    let mut native_options = eframe::NativeOptions::default();
    native_options.persist_window = false;
    native_options.persistence_path = homedir::my_home().map(|d| d.unwrap().join(SAVE_DIR).join("egui_persistence")).ok();
    native_options.viewport = native_options.viewport.with_inner_size((WINDOW_WIDTH, WINDOW_HEIGHT)).with_resizable(false).with_maximized(false).with_maximize_button(false);

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
                style.visuals.widgets.inactive.bg_stroke = (1.0, INACTIVE_BG_STROKE_COLOR).into();
                style.visuals.widgets.active.bg_stroke = (1.0, THEME_COLOUR).into();
            });
            egui_extras::install_image_loaders(&cc.egui_ctx);
            setup_custom_fonts(&cc.egui_ctx);
            Ok(Box::new(PedalboardClientApp::new(cc)))
        }
    )).expect("Failed to run app");
}


pub struct PedalboardClientApp {
    state: &'static State,

    selected_screen: usize,
    pedalboard_stage_screen: PedalboardStageScreen,
    pedalboard_library_screen: PedalboardLibraryScreen,
    utilities_screen: UtilitiesScreen,
    songs_screen: SongsScreen,
    settings_screen: SettingsScreen
}

impl PedalboardClientApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let loaded_state = State::load_state();
        let leaked_state = if let Err(e) = loaded_state {
            log::error!("Failed to load state. Using default state. Error: {}", e);
            Box::leak(Box::new(State::default()))
        } else {
            Box::leak(Box::new(loaded_state.unwrap()))
        };
        let _ = leaked_state.connect_to_server();

        let mut settings_screen = SettingsScreen::new(leaked_state);

        let no_server_start_arg = std::env::args().any(|arg| arg == "--no-server");
        // Start up the server process if configured to do so, not already connected and not running with the `--no-server` argument
        if leaked_state.client_settings.borrow().startup_server && !leaked_state.is_connected() && !no_server_start_arg {
            log::info!("Starting server on startup");
            if settings_screen.ready_to_start_server(&leaked_state.server_settings.borrow()) {
                match server_process::start_server_process(&leaked_state.server_settings.borrow()) {
                    Some(child) => {
                        settings_screen.server_launch_state = ServerLaunchState::AwaitingStart { start_time: Instant::now(), process: child };
                        loop {
                            settings_screen.handle_server_launch();
                            if !settings_screen.server_launch_state.is_awaiting() {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                    },
                    None => log::error!("Failed to start server process")
                }
            } else {
                log::error!("Set input and output device to launch server on start");
            }
        }

        PedalboardClientApp {
            selected_screen: 0,
            pedalboard_stage_screen: PedalboardStageScreen::new(leaked_state),
            pedalboard_library_screen: PedalboardLibraryScreen::new(leaked_state),
            songs_screen: SongsScreen::new(leaked_state),
            utilities_screen: UtilitiesScreen::new(leaked_state),
            settings_screen,
            state: leaked_state,
        }
    }
}

impl eframe::App for PedalboardClientApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.state.update_socket_responses();

        let mut sr_buf = Vec::new();
        self.state.get_commands("sr", &mut sr_buf);
        if !sr_buf.is_empty() {
            log::info!("Server is using sample rate: {}hz", sr_buf[0]);
        }

        let bottom_window_select_height = WINDOW_HEIGHT / 10.0;
        let padding = 10.0;
        egui::TopBottomPanel::bottom(Id::new("bottom_window_select"))
            .min_height(bottom_window_select_height)
            .show(&ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let mut button_outline = [egui::Stroke::new(0.3, INACTIVE_BG_STROKE_COLOR); 5];
                    button_outline[self.selected_screen] = egui::Stroke::new(1.0, THEME_COLOUR);
                    let mut button_bg = [egui::Color32::from_gray(19); 5];
                    button_bg[self.selected_screen] = egui::Color32::from_gray(33);

                    ui.allocate_ui(Vec2::new(ui.available_width()-(bottom_window_select_height*2.0), ui.available_height()), |ui| {
                        ui.columns_const(|[column0, column1, column2]| {
                            let button_size = [column0.available_width(), column0.available_height() - padding];
        
                            column0.horizontal_centered(|ui| {
                                if ui.add_sized(button_size, Button::new(
                                    RichText::new("Stage View").size(20.0)
                                ).stroke(button_outline[0]).fill(button_bg[0])).clicked() {
                                    if self.selected_screen == 2 {
                                        self.utilities_screen.tuner.active = false;
                                        self.state.set_tuner_active_server(false);
                                    }
                                    self.selected_screen = 0;
                                }
                            });
                            column1.horizontal_centered(|ui| {
                                if ui.add_sized(button_size, Button::new(
                                    RichText::new("Library").size(20.0)
                                ).stroke(button_outline[1]).fill(button_bg[1])).clicked() {
                                    if self.selected_screen == 2 {
                                        self.utilities_screen.tuner.active = false;
                                        self.state.set_tuner_active_server(false);
                                    }
                                    self.selected_screen = 1;
                                }
                            });
                            column2.horizontal_centered(|ui| {
                                if ui.add_sized(button_size, Button::new(
                                    RichText::new("Utilities").size(20.0)
                                ).stroke(button_outline[2]).fill(button_bg[2])).clicked() {
                                    self.utilities_screen.tuner.active = true;
                                    self.state.set_tuner_active_server(true);
                                    self.selected_screen = 2;
                                }
                            });
                        });
                    });

                    ui.add_space(padding/2.0);

                    // Smaller songs and settings buttons
                    // ImageButton doesnt have methods for stroke and fill, so we use style_mut() to set the style
                    ui.style_mut().visuals.widgets.inactive.weak_bg_fill = button_bg[3];
                    ui.style_mut().visuals.widgets.inactive.bg_stroke = button_outline[3];
                    if ui.add_sized(
                        Vec2::splat(bottom_window_select_height-padding-5.0), // why -5.0? idk
                        ImageButton::new(include_image!("files/songs_icon.png"))
                            .corner_radius(3.0)
                            .tint(Color32::from_white_alpha(200))
                    ).clicked() {
                        if self.selected_screen == 2 {
                            self.utilities_screen.tuner.active = false;
                            self.state.set_tuner_active_server(false);
                        }
                        self.selected_screen = 3;
                    }

                    ui.style_mut().visuals.widgets.inactive.weak_bg_fill = button_bg[4];
                    let settings_button_outline = if self.selected_screen == 4 {
                        button_outline[4]
                    } else {
                        if self.state.is_connected() {
                            button_outline[4]
                        } else {
                            egui::Stroke::new(2.5, Color32::RED)
                        }
                    };
                    ui.style_mut().visuals.widgets.inactive.bg_stroke = settings_button_outline;
                    if ui.add_sized(
                        Vec2::new(bottom_window_select_height, bottom_window_select_height-padding-5.0),
                        ImageButton::new(include_image!("files/settings_icon.png"))
                            .corner_radius(3.0)
                            .tint(Color32::from_white_alpha(200))
                    ).clicked() {
                        if self.selected_screen == 2 {
                            self.utilities_screen.tuner.active = false;
                            self.state.set_tuner_active_server(false);
                        }
                        self.selected_screen = 4;
                    };
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
                    ui.add(&mut self.utilities_screen);
                },
                3 => {
                    ui.add(&mut self.songs_screen);
                },
                4 => {
                    ui.add(&mut self.settings_screen);
                },
                _ => {
                    ui.label("Invalid screen selected");
                }
            };
        });
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        log::info!("Saving state");
        if let Err(e) = self.state.save_state() {
            log::error!("Failed to save state: {}", e);
        } else {
            log::info!("State saved successfully");
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.state.client_settings.borrow().kill_server_on_close {
            log::info!("Killing server on exit");
            self.state.kill_server();
        }
    }
}