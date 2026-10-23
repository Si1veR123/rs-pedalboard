use serde::{Serialize, Deserialize};

pub const LOOPER_AUDIO_SAVE_DIR: &str = "looper_audio";
pub const LOOPER_SETTINGS_FILE: &str = "looper_settings.json";

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MultitrackInformation {
    pub name: String,
    pub track_count: usize,
}

impl MultitrackInformation {
    pub fn try_from_name(name: String) -> Result<Self, String> {
        let dir_path = crate::get_save_dir()
            .map_err(|e| format!("Failed to get save directory: {}", e))?
            .join(LOOPER_AUDIO_SAVE_DIR)
            .join(&name);

        if !dir_path.exists() {
            return Err(format!("Directory for multitrack '{}' does not exist", name));
        }

        let mut track_count = 0;
        
        for entry in std::fs::read_dir(&dir_path).map_err(|e| format!("Failed to read directory: {}", e))? {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();
            if path.is_file()
                && path.extension().map_or(false, |ext| ext == "wav")
                && path.file_stem().map_or(false, |stem| !stem.to_string_lossy().ends_with("tail")) {
                track_count += 1;
            }
        }

        Ok(Self { name, track_count })
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LooperInformation {
    pub multitracks_information: Vec<MultitrackInformation>,
    pub settings: LooperSettings,
}

impl Default for LooperInformation {
    fn default() -> Self {
        Self {
            multitracks_information: Vec::new(),
            settings: LooperSettings::default(),
        }
    }
}

impl LooperInformation {
    pub fn from_disk() -> Result<Self, String> {
        let save_dir = crate::get_save_dir()
            .map_err(|e| format!("Failed to get save directory: {}", e))?
            .join(LOOPER_AUDIO_SAVE_DIR);

        if !save_dir.exists() {
            return Ok(Self::default());
        }

        let mut multitracks_information = Vec::new();

        for entry in std::fs::read_dir(&save_dir).map_err(|e| format!("Failed to read save directory: {}", e))? {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    match MultitrackInformation::try_from_name(name.to_string()) {
                        Ok(info) => multitracks_information.push(info),
                        Err(e) => tracing::warn!("Failed to load multitrack '{}': {}", name, e),
                    }
                }
            }
        }

        let settings = LooperSettings::load_from_disk().unwrap_or_else(|e| {
            tracing::warn!("Failed to load looper settings: {}", e);
            LooperSettings::default()
        });

        Ok(Self { multitracks_information, settings })
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LooperSync {
    pub bpm: u16,
    pub beats_per_bar: u8,
}

impl LooperSync {
    /// Return a sample length quantized according to the looper sync
    pub fn quantize(&self, sample_length: usize, sample_rate: f32) -> usize {
        let samples_per_beat = (sample_rate * 60.0 / self.bpm as f32) as usize;
        let samples_per_bar = samples_per_beat * self.beats_per_bar as usize;

        if sample_length < samples_per_bar {
            samples_per_bar
        } else {
            let bars = (sample_length + samples_per_bar - 1) / samples_per_bar; // Round up to the nearest bar
            bars * samples_per_bar
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LooperSettings {
    pub maximum_fractionals: usize,
    pub looper_sync: Option<LooperSync>
}

impl LooperSettings {
    pub fn save_to_disk(&self) -> Result<(), String> {
        let save_dir = crate::get_save_dir()
            .map_err(|e| format!("Failed to get save directory: {}", e))?
            .join(LOOPER_AUDIO_SAVE_DIR);

        if !save_dir.exists() {
            std::fs::create_dir_all(&save_dir)
                .map_err(|e| format!("Failed to create save directory: {}", e))?;
        }

        let settings_path = save_dir.join(LOOPER_SETTINGS_FILE);
        let settings_json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;
        std::fs::write(&settings_path, settings_json)
            .map_err(|e| format!("Failed to write settings file: {}", e))?;

        Ok(())
    }

    pub fn load_from_disk() -> Result<Self, String> {
        let save_dir = crate::get_save_dir()
            .map_err(|e| format!("Failed to get save directory: {}", e))?
            .join(LOOPER_AUDIO_SAVE_DIR);

        let settings_path = save_dir.join(LOOPER_SETTINGS_FILE);
        if !settings_path.exists() {
            return Ok(Self::default());
        }

        let settings_json = std::fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings file: {}", e))?;
        let settings: Self = serde_json::from_str(&settings_json)
            .map_err(|e| format!("Failed to deserialize settings: {}", e))?;

        Ok(settings)
    }
}

impl Default for LooperSettings {
    fn default() -> Self {
        Self {
            maximum_fractionals: 4,
            looper_sync: None
        }
    }
}
