use rs_pedalboard::pedals::PedalParameterValue;
use serde::{Serialize, Deserialize};
use strum_macros::EnumIter;

use crate::socket::Command;

#[derive(Debug, Clone, Serialize, Deserialize, EnumIter, PartialEq)]
pub enum GlobalMidiFunction {
    ToggleMute,
    SetMasterIn,
    SetMasterOut,
    NextPedalboard,
    PrevPedalboard,
    ToggleRecording,
    ToggleMetronome,
    DeleteActivePedalboard,
    StageView,
    LibraryView,
    UtilitiesView,
    SongsView,
    SettingsView,
    ChangeActiveParameter,
    ResetVolumeNormalization
}

impl std::fmt::Display for GlobalMidiFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            GlobalMidiFunction::ToggleMute => "Toggle Mute",
            GlobalMidiFunction::SetMasterIn => "Set Master In Volume",
            GlobalMidiFunction::SetMasterOut => "Set Master Out Volume",
            GlobalMidiFunction::NextPedalboard => "Next Pedalboard",
            GlobalMidiFunction::PrevPedalboard => "Previous Pedalboard",
            GlobalMidiFunction::ToggleRecording => "Toggle Recording",
            GlobalMidiFunction::ToggleMetronome => "Toggle Metronome",
            GlobalMidiFunction::DeleteActivePedalboard => "Delete Active Pedalboard",
            GlobalMidiFunction::StageView => "Stage View",
            GlobalMidiFunction::LibraryView => "Library View",
            GlobalMidiFunction::UtilitiesView => "Utilities View",
            GlobalMidiFunction::SongsView => "Songs View",
            GlobalMidiFunction::SettingsView => "Settings View",
            GlobalMidiFunction::ChangeActiveParameter => "Change Active Parameter",
            GlobalMidiFunction::ResetVolumeNormalization => "Reset Volume Normalization"
        };
        write!(f, "{name}")
    }
}

impl GlobalMidiFunction {
    pub fn command_from_function(&self, value: f32) -> Command {
        match self {
            GlobalMidiFunction::ToggleMute => Command::ToggleMute,
            GlobalMidiFunction::SetMasterIn => Command::MasterIn(value),
            GlobalMidiFunction::SetMasterOut => Command::MasterOut(value),
            GlobalMidiFunction::NextPedalboard => Command::NextPedalboard,
            GlobalMidiFunction::PrevPedalboard => Command::PrevPedalboard,
            GlobalMidiFunction::ToggleRecording => Command::ToggleRecording,
            GlobalMidiFunction::ToggleMetronome => Command::ToggleMetronome,
            GlobalMidiFunction::DeleteActivePedalboard => Command::DeleteActivePedalboard,
            GlobalMidiFunction::StageView => Command::StageView,
            GlobalMidiFunction::LibraryView => Command::LibraryView,
            GlobalMidiFunction::UtilitiesView => Command::UtilitiesView,
            GlobalMidiFunction::SongsView => Command::SongsView,
            GlobalMidiFunction::SettingsView => Command::SettingsView,
            GlobalMidiFunction::ChangeActiveParameter => Command::ChangeActiveParameter(value),
            GlobalMidiFunction::ResetVolumeNormalization => Command::VolumeNormalizationReset
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParameterMidiFunctionValues {
    pub min_value: PedalParameterValue,
    pub max_value: PedalParameterValue
}

impl ParameterMidiFunctionValues {
    pub fn parameter_from_value(&self, value: f32) -> PedalParameterValue {
        match self.min_value {
            PedalParameterValue::Float(min) => {
                let max = self.max_value.as_float().unwrap_or(min);
                PedalParameterValue::Float(min + (max - min) * value)
            },
            PedalParameterValue::Int(min) => {
                let max = self.max_value.as_int().unwrap_or(min);
                PedalParameterValue::Int(min + ((max - min) as f32 * value).round() as i16)
            },
            PedalParameterValue::Bool(_) |
            PedalParameterValue::Oscillator(_) |
            PedalParameterValue::String(_) => {
                if value >= 0.5 { self.max_value.clone() } else { self.min_value.clone() }
            }
        }
    }
}