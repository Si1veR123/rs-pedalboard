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
use tracing::trace_span;
use utilities::UtilitiesScreen;
mod settings;
use settings::{SettingsScreen, ServerLaunchState};
mod server_process;
mod midi;

#[cfg(feature = "virtual_keyboard")]
use egui_keyboard::{Keyboard, layouts::KeyboardLayout};

use eframe::egui::{self, include_image, Button, Color32, FontId, Id, ImageButton, RichText, Vec2, FontFamily};
use rs_pedalboard::SAVE_DIR;
use std::{sync::Arc, time::Instant, io, fs::File};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, filter::EnvFilter, Layer};

const SERVER_PORT: u16 = 29475;
const WINDOW_HEIGHT: f32 = 1080.0;
const WINDOW_WIDTH: f32 = 1920.0;

pub const THEME_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 105, 46);
pub const FAINT_THEME_COLOR_ALPHA: f32 = 0.5;
pub const ROW_COLOR_LIGHT: egui::Color32 = egui::Color32::from_gray(28);
pub const ROW_COLOR_DARK: egui::Color32 = egui::Color32::from_gray(22);
pub const TEXT_COLOR: egui::Color32 = egui::Color32::from_gray(200);
pub const FAINT_TEXT_COLOR: egui::Color32 = egui::Color32::from_gray(130);
pub const EXTREME_BACKGROUND_COLOR: egui::Color32 = egui::Color32::from_gray(2);
pub const BACKGROUND_COLOR: egui::Color32 = egui::Color32::from_gray(15);
pub const LIGHT_BACKGROUND_COLOR: egui::Color32 = egui::Color32::from_gray(22);
pub const WIDGET_BACKGROUND_COLOR: egui::Color32 = egui::Color32::from_gray(34);
pub const WIDGET_HOVER_BACKGROUND_COLOR: egui::Color32 = egui::Color32::from_gray(40);
pub const WIDGET_CLICK_BACKGROUND_COLOR_THEME_ALPHA: f32 = 0.025;
// Buttons
pub const INACTIVE_BG_STROKE_COLOR: egui::Color32 = egui::Color32::from_gray(54);

fn set_font_size(width: f32, ctx: &egui::Context) {
    let base_size = (width / 1920.0) * 18.0;

    let mut style = (*ctx.style()).clone();
    let text_styles = [
        (egui::TextStyle::Heading, FontId::new(base_size * 2.0, FontFamily::Proportional)),
        (egui::TextStyle::Body, FontId::new(base_size*1.38, FontFamily::Proportional)),
        (egui::TextStyle::Monospace, FontId::new(base_size*1.33, FontFamily::Monospace)),
        (egui::TextStyle::Button, FontId::new(base_size*1.38, FontFamily::Proportional)),
        (egui::TextStyle::Small, FontId::new(base_size, FontFamily::Proportional)),
    ];

    for (text_style, font_id) in text_styles {
        style.text_styles.insert(text_style, font_id);
    }

    ctx.set_style(style);
}

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
}

pub fn init_tracing() {
    // Console layer
    let console_filter_layer = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let stdout_layer = fmt::layer()
        .with_writer(io::stdout)
        .with_timer(rs_pedalboard::TimeOnlyFormat)
        .with_target(false)
        .with_filter(console_filter_layer);

    // File layer
    let file_filter_layer = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("debug"));

    let file = File::create("pedalboard-client.log")
        .expect("Failed to create log file");
    let file_layer = fmt::layer()
        .with_writer(file)
        .with_ansi(false)
        .with_target(true)
        .with_filter(file_filter_layer);

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .init();
}

pub fn init_panic_logging() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("panic: {info:?}");
        default_hook(info);
    }));
}

fn main() {
    init_tracing();
    tracing::info!("Started logging...");
    init_panic_logging();

    let mut native_options = eframe::NativeOptions::default();
    native_options.persist_window = false;
    native_options.persistence_path = homedir::my_home().map(|d| d.unwrap().join(SAVE_DIR).join("egui_persistence")).ok();
    native_options.viewport = native_options.viewport.with_inner_size((WINDOW_WIDTH, WINDOW_HEIGHT)).with_maximized(true).with_maximize_button(true);

    eframe::run_native("Pedalboard", native_options, Box::new(
        |cc| {
            cc.egui_ctx.style_mut(|style| {
                style.visuals.extreme_bg_color = EXTREME_BACKGROUND_COLOR.into();
                style.visuals.panel_fill = BACKGROUND_COLOR.into();
                style.visuals.override_text_color = Some(TEXT_COLOR.into());
                style.visuals.extreme_bg_color = EXTREME_BACKGROUND_COLOR.into();
                let widget_click_background_color = THEME_COLOR.gamma_multiply(WIDGET_CLICK_BACKGROUND_COLOR_THEME_ALPHA);
                style.visuals.widgets.active.bg_fill = widget_click_background_color.into();
                style.visuals.widgets.active.weak_bg_fill = widget_click_background_color.into();
                let faint_theme_color = THEME_COLOR.gamma_multiply(FAINT_THEME_COLOR_ALPHA);
                style.visuals.selection.bg_fill = faint_theme_color;
                style.visuals.widgets.hovered.bg_fill = WIDGET_HOVER_BACKGROUND_COLOR.into();
                style.visuals.widgets.hovered.weak_bg_fill = WIDGET_HOVER_BACKGROUND_COLOR.into();
                style.visuals.widgets.inactive.bg_fill = WIDGET_BACKGROUND_COLOR.into();
                style.visuals.widgets.inactive.weak_bg_fill = WIDGET_BACKGROUND_COLOR.into();
                style.visuals.widgets.inactive.bg_stroke = (1.0, INACTIVE_BG_STROKE_COLOR).into();
                style.visuals.widgets.active.bg_stroke = (1.0, THEME_COLOR).into();
            });
            egui_extras::install_image_loaders(&cc.egui_ctx);
            setup_custom_fonts(&cc.egui_ctx);
            Ok(Box::new(PedalboardClientApp::new(cc)))
        }
    )).expect("Failed to run app");
}


pub struct PedalboardClientApp {
    state: &'static State,

    #[cfg(feature = "virtual_keyboard")]
    keyboard: Keyboard,

    selected_screen: usize,
    pedalboard_stage_screen: PedalboardStageScreen,
    pedalboard_library_screen: PedalboardLibraryScreen,
    utilities_screen: UtilitiesScreen,
    songs_screen: SongsScreen,
    settings_screen: SettingsScreen
}

impl PedalboardClientApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let loaded_state = State::load_state(cc.egui_ctx.clone());
        let leaked_state = Box::leak(Box::new(loaded_state));
        let _ = leaked_state.connect_to_server();

        let mut settings_screen = SettingsScreen::new(leaked_state);

        let no_server_start_arg = std::env::args().any(|arg| arg == "--no-server");
        // Start up the server process if configured to do so, not already connected and not running with the `--no-server` argument
        if leaked_state.client_settings.borrow().startup_server && !leaked_state.is_connected() && !no_server_start_arg {
            tracing::info!("Starting server on startup");
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
                    None => tracing::error!("Failed to start server process")
                }
            } else {
                tracing::error!("Set input and output device to launch server on start");
            }
        }

        // Linux (JACK) requires jack server to be running before connecting MIDI ports
        // This is started by the server app
        leaked_state.midi_state.borrow_mut().connect_to_auto_connect_ports();

        PedalboardClientApp {
            selected_screen: 0,
            pedalboard_stage_screen: PedalboardStageScreen::new(leaked_state),
            pedalboard_library_screen: PedalboardLibraryScreen::new(leaked_state),
            songs_screen: SongsScreen::new(leaked_state),
            utilities_screen: UtilitiesScreen::new(leaked_state),
            settings_screen,
            state: leaked_state,
            #[cfg(feature = "virtual_keyboard")]
            keyboard: Keyboard::default().layout(KeyboardLayout::Qwerty),
        }
    }
}

impl eframe::App for PedalboardClientApp {
    #[tracing::instrument(level = "trace", skip_all)]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(feature = "virtual_keyboard")]
        {
            self.keyboard.pump_events(ctx);
            self.keyboard.show(ctx);
        }

        set_font_size(ctx.available_rect().width(), ctx);

        self.state.update_socket_responses();
        self.state.handle_other_thread_commands();

        let mut sr_buf = Vec::new();
        self.state.get_commands("sr", &mut sr_buf);
        if !sr_buf.is_empty() {
            tracing::info!("Server is using sample rate: {}hz", sr_buf[0]);
        }

        let bottom_window_select_height = ctx.screen_rect().height() * 0.1;
        let padding = 10.0;

        let span = trace_span!("TopBottomPanel");
        let enter = span.enter();
        egui::TopBottomPanel::bottom(Id::new("bottom_window_select"))
            .min_height(bottom_window_select_height)
            .show(&ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let mut button_outline = [egui::Stroke::new(0.3, INACTIVE_BG_STROKE_COLOR); 5];
                    button_outline[self.selected_screen] = egui::Stroke::new(1.0, THEME_COLOR);
                    let mut button_bg = [egui::Color32::from_gray(19); 5];
                    button_bg[self.selected_screen] = egui::Color32::from_gray(33);

                    ui.allocate_ui(Vec2::new(ui.available_width()-(bottom_window_select_height*2.0), ui.available_height()), |ui| {
                        ui.columns_const(|[column0, column1, column2]| {
                            let button_size = [column0.available_width(), column0.available_height() - padding];

                            column0.horizontal_centered(|ui| {
                                if ui.add_sized(button_size, Button::new(
                                    RichText::new("Stage View")
                                ).stroke(button_outline[0]).fill(button_bg[0])).clicked() {
                                    if self.selected_screen == 2 {
                                        self.state.set_tuner_active(false);
                                    }
                                    self.selected_screen = 0;
                                }
                            });
                            column1.horizontal_centered(|ui| {
                                if ui.add_sized(button_size, Button::new(
                                    RichText::new("Library")
                                ).stroke(button_outline[1]).fill(button_bg[1])).clicked() {
                                    if self.selected_screen == 2 {
                                        self.state.set_tuner_active(false);
                                    }
                                    self.selected_screen = 1;
                                }
                            });
                            column2.horizontal_centered(|ui| {
                                let recording = self.state.recording_time.get().is_some();
                                let text_color = if recording {
                                    ui.visuals().text_color().lerp_to_gamma(Color32::RED, 0.5)
                                } else {
                                    ui.visuals().text_color()
                                };

                                if ui.add_sized(button_size, Button::new(
                                    RichText::new("Utilities").color(text_color)
                                ).stroke(button_outline[2]).fill(button_bg[2])).clicked() {
                                    self.state.set_tuner_active(true);
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
                            self.state.set_tuner_active(false);
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
                            self.state.set_tuner_active(false);
                        }
                        self.selected_screen = 4;
                    };
                });
        });
        drop(enter);

        let span = trace_span!("CentralPanel", screen = self.selected_screen);
        let enter = span.enter();
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
        drop(enter);
    }

    #[tracing::instrument(level = "debug", skip_all)]
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        // Remove any MIDI parameter functions that refer to pedalboards that no longer exist
        self.state.midi_state.borrow_mut().remove_old_parameter_functions(&self.state.all_pedalboard_ids());

        tracing::info!("Saving state");
        if let Err(e) = self.state.save_state() {
            tracing::error!("Failed to save state: {}", e);
        } else {
            tracing::info!("State saved successfully");
        }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.state.client_settings.borrow().kill_server_on_close {
            tracing::info!("Killing server on exit");
            self.state.kill_server();
        }
    }
}