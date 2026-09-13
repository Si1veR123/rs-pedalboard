use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::{midi::MidiChange, socket::Command};

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
    ResetVolumeNormalization,
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
            GlobalMidiFunction::ResetVolumeNormalization => "Reset Volume Normalization",
        };
        write!(f, "{name}")
    }
}

impl GlobalMidiFunction {
    pub fn command_from_function(&self, change: &MidiChange) -> Option<Command> {
        let float_setting_update = change.to_float_setting_update();
        match float_setting_update {
            None => return None,
            Some(float_setting_update) => Some(match self {
                GlobalMidiFunction::ToggleMute => Command::ToggleMute,
                GlobalMidiFunction::SetMasterIn => Command::MasterIn(float_setting_update),
                GlobalMidiFunction::SetMasterOut => Command::MasterOut(float_setting_update),
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
                GlobalMidiFunction::ChangeActiveParameter => {
                    Command::ChangeActiveParameter(float_setting_update)
                }
                GlobalMidiFunction::ResetVolumeNormalization => Command::VolumeNormalizationReset,
            }),
        }
    }
}
