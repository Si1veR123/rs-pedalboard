use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::{
    midi::device::MidiChange,
    socket::{Command, LooperSubcommand},
};

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
    ActiveFeature,
    ResetVolumeNormalization,
    NextInputEQ,
    NextOutputEQ,
    PrevInputEQ,
    PrevOutputEQ,
    ToggleLooperMode,
    LooperModeAddTrackToActiveMultitrack,
    LooperModePrevActiveMultitrack,
    LooperModeNextActiveMultitrack,
    LooperModePrevSelectedTrack,
    LooperModeNextSelectedTrack,
    LooperModeTogglePlaySelectedTrack,
    LooperModeToggleRecordSelectedTrack,
    LooperModePlayAll,
    LooperModeStopAll,
    LooperModeStopAllImmediately,
    LooperModeRemoveTrack,
    LooperModeClearTrack,
    LooperModeReverseEffect,
    LooperModeHalfSpeedEffect,
    LooperModeDoubleSpeedEffect,
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
            GlobalMidiFunction::ActiveFeature => "Active Feature",
            GlobalMidiFunction::ResetVolumeNormalization => "Reset Volume Normalization",
            GlobalMidiFunction::NextInputEQ => "Next Input EQ",
            GlobalMidiFunction::NextOutputEQ => "Next Output EQ",
            GlobalMidiFunction::PrevInputEQ => "Previous Input EQ",
            GlobalMidiFunction::PrevOutputEQ => "Previous Output EQ",
            GlobalMidiFunction::ToggleLooperMode => "Toggle Looper Mode",
            GlobalMidiFunction::LooperModeAddTrackToActiveMultitrack => "Looper Mode: Add Track",
            GlobalMidiFunction::LooperModePrevActiveMultitrack => {
                "Looper Mode: Previous Active Multitrack"
            }
            GlobalMidiFunction::LooperModeNextActiveMultitrack => "Looper Mode: Next Active Multitrack",
            GlobalMidiFunction::LooperModePrevSelectedTrack => {
                "Looper Mode: Previous Selected Track"
            }
            GlobalMidiFunction::LooperModeNextSelectedTrack => "Looper Mode: Next Selected Track",
            GlobalMidiFunction::LooperModeTogglePlaySelectedTrack => {
                "Looper Mode: Toggle Play Selected Track"
            }
            GlobalMidiFunction::LooperModeToggleRecordSelectedTrack => {
                "Looper Mode: Toggle Record Selected Track"
            }
            GlobalMidiFunction::LooperModePlayAll => "Looper Mode: Play All",
            GlobalMidiFunction::LooperModeStopAll => "Looper Mode: Stop All",
            GlobalMidiFunction::LooperModeStopAllImmediately => "Looper Mode: Stop All Immediately",
            GlobalMidiFunction::LooperModeRemoveTrack => "Looper Mode: Remove Track",
            GlobalMidiFunction::LooperModeClearTrack => "Looper Mode: Clear Track",
            GlobalMidiFunction::LooperModeReverseEffect => "Looper Mode: Reverse Effect",
            GlobalMidiFunction::LooperModeHalfSpeedEffect => "Looper Mode: Half Speed Effect",
            GlobalMidiFunction::LooperModeDoubleSpeedEffect => "Looper Mode: Double Speed Effect",
        };
        write!(f, "{name}")
    }
}

impl GlobalMidiFunction {
    pub fn command_from_function(&self, change: &MidiChange) -> Option<Command> {
        let float_setting_update = change.to_float_setting_update();
        match float_setting_update {
            // Toggle looper mode is special and handled separately
            None => return None,
            Some(float_setting_update) => {
                let command = match self {
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
                    GlobalMidiFunction::ActiveFeature => {
                        Command::ChangeActiveFeature(float_setting_update)
                    }
                    GlobalMidiFunction::ResetVolumeNormalization => {
                        Command::VolumeNormalizationReset
                    }
                    GlobalMidiFunction::NextInputEQ => Command::NextGlobalInputEQ,
                    GlobalMidiFunction::NextOutputEQ => Command::NextGlobalOutputEQ,
                    GlobalMidiFunction::PrevInputEQ => Command::PrevGlobalInputEQ,
                    GlobalMidiFunction::PrevOutputEQ => Command::PrevGlobalOutputEQ,
                    GlobalMidiFunction::LooperModeAddTrackToActiveMultitrack => {
                        Command::Looper(LooperSubcommand::AddTrackToActiveMultitrack)
                    }
                    GlobalMidiFunction::LooperModePrevActiveMultitrack => {
                        Command::Looper(LooperSubcommand::PrevActiveMultitrack)
                    }
                    GlobalMidiFunction::LooperModeNextActiveMultitrack => {
                        Command::Looper(LooperSubcommand::NextActiveMultitrack)
                    }
                    GlobalMidiFunction::LooperModePrevSelectedTrack => {
                        Command::Looper(LooperSubcommand::PrevSelectedTrack)
                    }
                    GlobalMidiFunction::LooperModeNextSelectedTrack => {
                        Command::Looper(LooperSubcommand::NextSelectedTrack)
                    }
                    GlobalMidiFunction::LooperModeTogglePlaySelectedTrack => {
                        Command::Looper(LooperSubcommand::TogglePlaySelectedTrack)
                    }
                    GlobalMidiFunction::LooperModeToggleRecordSelectedTrack => {
                        Command::Looper(LooperSubcommand::ToggleRecordSelectedTrack)
                    }
                    GlobalMidiFunction::LooperModePlayAll => {
                        Command::Looper(LooperSubcommand::PlayAll)
                    }
                    GlobalMidiFunction::LooperModeStopAll => {
                        Command::Looper(LooperSubcommand::StopAll)
                    }
                    GlobalMidiFunction::LooperModeStopAllImmediately => {
                        Command::Looper(LooperSubcommand::StopAllImmediately)
                    }
                    GlobalMidiFunction::LooperModeRemoveTrack => {
                        Command::Looper(LooperSubcommand::RemoveTrack)
                    }
                    GlobalMidiFunction::LooperModeClearTrack => {
                        Command::Looper(LooperSubcommand::ClearTrack)
                    }
                    GlobalMidiFunction::LooperModeReverseEffect => {
                        Command::Looper(LooperSubcommand::ReverseEffect)
                    }
                    GlobalMidiFunction::LooperModeHalfSpeedEffect => {
                        Command::Looper(LooperSubcommand::HalfSpeedEffect)
                    }
                    GlobalMidiFunction::LooperModeDoubleSpeedEffect => {
                        Command::Looper(LooperSubcommand::DoubleSpeedEffect)
                    }
                    GlobalMidiFunction::ToggleLooperMode => return None,
                };

                Some(command)
            }
        }
    }

    pub fn is_looper_mode_function(&self) -> bool {
        matches!(
            self,
            GlobalMidiFunction::LooperModeAddTrackToActiveMultitrack
                | GlobalMidiFunction::LooperModePrevActiveMultitrack
                | GlobalMidiFunction::LooperModeNextActiveMultitrack
                | GlobalMidiFunction::LooperModePrevSelectedTrack
                | GlobalMidiFunction::LooperModeNextSelectedTrack
                | GlobalMidiFunction::LooperModeTogglePlaySelectedTrack
                | GlobalMidiFunction::LooperModeToggleRecordSelectedTrack
                | GlobalMidiFunction::LooperModePlayAll
                | GlobalMidiFunction::LooperModeStopAll
                | GlobalMidiFunction::LooperModeStopAllImmediately
                | GlobalMidiFunction::LooperModeRemoveTrack
                | GlobalMidiFunction::LooperModeClearTrack
                | GlobalMidiFunction::LooperModeReverseEffect
                | GlobalMidiFunction::LooperModeHalfSpeedEffect
                | GlobalMidiFunction::LooperModeDoubleSpeedEffect
        )
    }
}
