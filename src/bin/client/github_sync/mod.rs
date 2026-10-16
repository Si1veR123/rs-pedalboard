mod git_commands;
use git_commands::{git_clone, add_all_files_to_git, git_commit, git_push};
pub use git_commands::git_exists;
mod backup_settings;
use backup_settings::{clear_directory_contents, create_directory_backup, restore_directory_backup};

use serde::{Deserialize, Serialize};
use rs_pedalboard::{
    processor_settings::SAVE_NAME as PROCESSOR_SETTINGS_SAVE_NAME, processor_settings::RECORDINGS_SAVE_DIR,
    pedals::{NAM_SAVE_PATH, IR_SAVE_PATH}
};
use crate::{github_sync::backup_settings::partial_restore_directory_backup, popup_message::{PopupMessageType, popup}, saved_pedalboards::SAVE_NAME as PEDALBOARDS_SAVE_NAME, settings::CLIENT_SAVE_NAME};
use tracing::{error, warn, info};
use eframe::egui::{self, Widget};

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, MutexGuard, TryLockError};

const GITHUB_SYNC_SETTINGS_FILE: &str = "github_sync_settings.json";
const GITIGNORE_PERMANENT: &str = concat!("egui_persistence\n", "midi_settings.json\n", "github_sync_settings.json\n");
const GITHUB_ROOT_URL: &str = "https://github.com/";

static GIT_OPERATIONS_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

fn lock_git() -> Option<MutexGuard<'static, ()>> {
    let try_guard = GIT_OPERATIONS_MUTEX
        .get_or_init(|| Mutex::new(()))
        .try_lock();

    match try_guard {
        Ok(guard) => Some(guard),
        Err(e) => {
            match e {
                TryLockError::Poisoned(_) => {
                    error!("Git operations mutex is poisoned.");
                    None
                }
                TryLockError::WouldBlock => {
                    warn!("Cannot acquire lock: git operations are currently in progress.");
                    None
                }
            }
        }
    }
}

fn replace_working_directory_with_settings_repo(github_repo: &str) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Failed to get current working directory: {}", e))?;
    create_directory_backup(&cwd)?;
    clear_directory_contents(&cwd)?;

    let github_repo_link = format!("{}{}", GITHUB_ROOT_URL, github_repo);

    if let Err(e) = git_clone(&github_repo_link, &cwd) {
        restore_directory_backup(&cwd)?;
        return Err(format!("Failed to clone GitHub repository: {}", e));
    }

    Ok(())
}

pub struct SyncSettingsResponse {
    pub egui_response: egui::Response,
    pub push_triggered: bool,
    pub pull_triggered: bool
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GithubSyncSettings {
    pub is_auto_sync_enabled: bool,
    pub github_repo: String,
    pub sync_processor_settings: bool,
    pub sync_client_settings: bool,
    pub sync_nam_directory: bool,
    pub sync_ir_directory: bool,
    pub sync_recordings_directory: bool,
    pub sync_pedalboards: bool
}

impl Default for GithubSyncSettings {
    fn default() -> Self {
        Self {
            is_auto_sync_enabled: false,
            github_repo: String::new(),
            sync_processor_settings: false,
            sync_client_settings: true,
            sync_nam_directory: true,
            sync_ir_directory: true,
            sync_recordings_directory: true,
            sync_pedalboards: true
        }
    }
}

impl GithubSyncSettings {
    pub fn load_or_default() -> Self {
        let settings_path = rs_pedalboard::get_save_dir()
            .map(|dir| dir.join(GITHUB_SYNC_SETTINGS_FILE))
            .unwrap_or_else(|e| {
                warn!("Failed to get save directory: {}. Using default settings.", e);
                PathBuf::from(GITHUB_SYNC_SETTINGS_FILE)
            });

        if settings_path.exists() {
            match std::fs::read_to_string(settings_path) {
                Ok(contents) => match serde_json::from_str::<Self>(&contents) {
                    Ok(settings) => settings,
                    Err(e) => {
                        warn!("Failed to parse GitHub sync settings: {}. Using default settings.", e);
                        Self::default()
                    }
                },
                Err(e) => {
                    warn!("Failed to read GitHub sync settings: {}. Using default settings.", e);
                    Self::default()
                }
            }
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let settings_path = rs_pedalboard::get_save_dir()
            .map(|dir| dir.join(GITHUB_SYNC_SETTINGS_FILE))?;

        let serialized = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        std::fs::write(settings_path, serialized)?;

        Ok(())
    }

    fn validate_github_repo(&self) -> bool {
        // Should be in the format "username/repo"
        let parts: Vec<&str> = self.github_repo.split('/').collect();
        if parts.len() != 2 {
            return false;
        }
        // Check that both parts are non-empty and contain only valid characters
        for part in parts {
            if part.is_empty() || !part.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                return false;
            }
        }
        true
    }

    fn sync_selectable_label(ui: &mut egui::Ui, value: &mut bool, label: &str) {
        ui.selectable_label(*value, label).clicked().then(|| *value = !*value);
    }

    pub fn settings_ui(&mut self, ui: &mut egui::Ui, row_height: f32) -> SyncSettingsResponse {
        let mut push_triggered = false;
        let mut pull_triggered = false;

        let egui_response = egui::Grid::new("processor_settings_grid")
            .num_columns(2)
            .min_col_width(ui.available_width()*0.5)
            .min_row_height(row_height)
            .striped(true)
            .show(ui, |ui| {
                ui.label("GitHub Repo");

                let default_text_edit_bg_color = ui.visuals().text_edit_bg_color();
                let text_edit_bg_color = if self.validate_github_repo() {
                    default_text_edit_bg_color
                } else {
                    default_text_edit_bg_color.lerp_to_gamma(egui::Color32::RED, 0.3)
                };

                egui::TextEdit::singleline(&mut self.github_repo)
                    .hint_text("username/repo")
                    .background_color(text_edit_bg_color)
                    .ui(ui);
                ui.end_row();

                ui.label("Enable Auto Sync");
                ui.checkbox(&mut self.is_auto_sync_enabled, "");
                ui.end_row();

                ui.label("Synced Settings");
                egui::ComboBox::from_id_salt("synced_settings_combobox")
                    .selected_text("")
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show_ui(ui, |ui| {
                        Self::sync_selectable_label(ui, &mut self.sync_processor_settings, "Processor Settings");
                        Self::sync_selectable_label(ui, &mut self.sync_client_settings, "Client Settings");
                        Self::sync_selectable_label(ui, &mut self.sync_nam_directory, "NAM Directory");
                        Self::sync_selectable_label(ui, &mut self.sync_ir_directory, "IR Directory");
                        Self::sync_selectable_label(ui, &mut self.sync_recordings_directory, "Recordings Directory");
                        Self::sync_selectable_label(ui, &mut self.sync_pedalboards, "Pedalboards");
                    });
                
                ui.end_row();

                ui.label("");
                if ui.add_sized(ui.available_size_before_wrap() * 0.75, egui::Button::new("Push")).clicked() {
                    push_triggered = true;
                }
                ui.end_row();

                ui.label("");
                
                let is_currently_pulling_on_start = ui.ctx().data_mut(|d| d.get_persisted(egui::Id::from("force_pull_on_start")).unwrap_or(false));
                let button_fill = if is_currently_pulling_on_start {
                    ui.visuals().widgets.active.bg_fill
                } else {
                    ui.visuals().widgets.inactive.bg_fill
                };
                if ui.add_sized(
                    ui.available_size_before_wrap() * 0.75,
                    egui::Button::new("Pull On Next Startup").fill(button_fill)
                ).clicked() {
                    pull_triggered = true;
                }
                ui.end_row();
            }).response;
        
        SyncSettingsResponse {
            egui_response,
            push_triggered,
            pull_triggered
        }
    }

    fn construct_ignores(&self) -> String {
        let mut gitignore = String::from(GITIGNORE_PERMANENT);
        if !self.sync_processor_settings {
            gitignore.push_str(PROCESSOR_SETTINGS_SAVE_NAME);
            gitignore.push('\n');
        }
        if !self.sync_client_settings {
            gitignore.push_str(CLIENT_SAVE_NAME);
            gitignore.push('\n');
        }
        if !self.sync_nam_directory {
            gitignore.push_str(NAM_SAVE_PATH);
            gitignore.push_str("/\n");
        }
        if !self.sync_ir_directory {
            gitignore.push_str(IR_SAVE_PATH);
            gitignore.push_str("/\n");
        }
        if !self.sync_recordings_directory {
            gitignore.push_str(RECORDINGS_SAVE_DIR);
            gitignore.push_str("/\n");
        }
        if !self.sync_pedalboards {
            gitignore.push_str(PEDALBOARDS_SAVE_NAME);
            gitignore.push('\n');
        }
        gitignore
    }

    fn write_ignores_to_exclude(&self) -> Result<(), String> {
        let exclude_path = Path::new(".git/info/exclude");
        let mut file = std::fs::File::create(exclude_path).map_err(|e| format!("Failed to create .git/info/exclude file: {}", e))?;
        let ignores = self.construct_ignores();
        std::io::Write::write_all(&mut file, ignores.as_bytes()).map_err(|e| format!("Failed to write to .git/info/exclude file: {}", e))?;
        Ok(())
    }

    fn set_working_directory(&self) -> Result<PathBuf, String> {
        let current_working_directory = std::env::current_dir().expect("Failed to get current working directory");
        let new_working_directory = rs_pedalboard::get_save_dir().map_err(|e| format!("Failed to get save directory: {}", e))?;
        std::env::set_current_dir(new_working_directory).map_err(|e| format!("Failed to set working directory: {}", e))?;
        Ok(current_working_directory)
    }

    fn restore_working_directory(&self, previous_working_directory: impl AsRef<Path>) -> Result<(), String> {
        std::env::set_current_dir(previous_working_directory).map_err(|e| format!("Failed to restore working directory: {}", e))
    }

    fn restore_working_directory_with_handling(&self, previous_working_directory: impl AsRef<Path>, ctx: &egui::Context) {
        if let Err(e) = self.restore_working_directory(previous_working_directory) {
            // Warning since program can likely continue, but user should be aware that working directory was not restored
            warn!(e);
            popup!(ctx, e, None, PopupMessageType::Warning);
        }
    }

    fn ready_to_sync(&self, is_auto: bool) -> Result<bool, String> {
        if !self.is_auto_sync_enabled && is_auto {
            return Ok(false);
        }
        if !self.validate_github_repo() {
            return Err("Invalid GitHub repository format. Please use 'username/repo' format.".to_string());
        }
        if !git_exists() {
            return Err("Git not found in PATH. Please install Git to use GitHub sync features.".to_string());
        }
        Ok(true)
    }

    pub fn pull_changes_blocking(&self, ctx: &egui::Context, is_auto: bool) {
        let _guard = match lock_git() {
            Some(guard) => guard,
            None => return
        };

        match self.ready_to_sync(is_auto) {
            Ok(true) => {}
            Ok(false) => {
                info!("GitHub sync is disabled, skipping pull");
                return;
            }
            Err(e) => {
                error!(e);
                popup!(ctx, e, None, PopupMessageType::Error);
                return;
            }
        }

        let old_working_directory = match self.set_working_directory() {
            Ok(dir) => dir,
            Err(e) => {
                error!(e);
                popup!(ctx, e, None, PopupMessageType::Error);
                return;
            }
        };

        if let Err(e) = replace_working_directory_with_settings_repo(&self.github_repo) {
            error!(e);
            popup!(ctx, e, None, PopupMessageType::Error);
            self.restore_working_directory_with_handling(old_working_directory, ctx);
            return;
        }
        info!("Successfully pulled changes from GitHub repository: {}", self.github_repo);
        popup!(ctx, "Successfully pulled changes from GitHub repository", None, PopupMessageType::Info);

        // Restore settings which are not synced from the backup
        let save_dir = rs_pedalboard::get_save_dir().expect("Failed to get save directory");
        if let Err(e) = partial_restore_directory_backup(&save_dir, self) {
            error!(e);
            popup!(ctx, e, None, PopupMessageType::Error);
        }

        self.restore_working_directory_with_handling(old_working_directory, ctx);
    }    

    pub fn pull_changes(&self, ctx: &egui::Context, is_auto: bool) {
        std::thread::spawn({
            let self_clone = self.clone();
            let ctx_clone = ctx.clone();
            move || {
                self_clone.pull_changes_blocking(&ctx_clone, is_auto);
            }
        });
    }

    pub fn push_changes_blocking(&self, ctx: &egui::Context, is_auto: bool) {
        let _guard = match lock_git() {
            Some(guard) => guard,
            None => return
        };

        match self.ready_to_sync(is_auto) {
            Ok(true) => {}
            Ok(false) => {
                info!("GitHub sync is disabled, skipping push");
                return;
            }
            Err(e) => {
                error!(e);
                popup!(ctx, e, None, PopupMessageType::Error);
                return;
            }
        }

        let old_working_directory = match self.set_working_directory() {
            Ok(dir) => dir,
            Err(e) => {
                error!(e);
                popup!(ctx, e, None, PopupMessageType::Error);
                return;
            }
        };

        match std::fs::exists(".git") {
            Ok(true) => {},
            _ => {
                self.restore_working_directory_with_handling(old_working_directory, ctx);
                return;
            }
        }

        if let Err(e) = self.write_ignores_to_exclude() {
            error!(e);
            popup!(ctx, e, None, PopupMessageType::Error);
            self.restore_working_directory_with_handling(old_working_directory, ctx);
            return;
        }

        if let Err(e) = add_all_files_to_git() {
            error!(e);
            popup!(ctx, e, None, PopupMessageType::Error);
            self.restore_working_directory_with_handling(old_working_directory, ctx);
            return;
        }

        match git_commit("Sync changes from rs_pedalboard") {
            Ok(false) => {
                info!("No changes to commit");
            }
            Ok(true) => {
                if let Err(e) = git_push() {
                    error!(e);
                    popup!(ctx, e, None, PopupMessageType::Error);
                }
            }
            Err(e) => {
                error!(e);
                popup!(ctx, e, None, PopupMessageType::Error);
            }
        }

        self.restore_working_directory_with_handling(old_working_directory, ctx);
    }

    pub fn push_changes(&self, ctx: &egui::Context, is_auto: bool) {
        std::thread::spawn({
            let self_clone = self.clone();
            let ctx_clone = ctx.clone();
            move || {
                self_clone.push_changes_blocking(&ctx_clone, is_auto);
            }
        });
    }
}
