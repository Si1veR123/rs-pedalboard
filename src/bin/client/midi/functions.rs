use rs_pedalboard::pedals::PedalParameterValue;
use serde::{Serialize, Deserialize};
use strum_macros::EnumIter;

use crate::socket::Command;

#[derive(Debug, Clone, Serialize, Deserialize, EnumIter, PartialEq)]
pub enum GlobalMidiFunction {
    ToggleMute,
    SetMasterOut,
    NextPedalboard,
    PrevPedalboard,
    ToggleRecording,
    ToggleMetronome
}

impl std::fmt::Display for GlobalMidiFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            GlobalMidiFunction::ToggleMute => "Toggle Mute",
            GlobalMidiFunction::SetMasterOut => "Set Master Out Volume",
            GlobalMidiFunction::NextPedalboard => "Next Pedalboard",
            GlobalMidiFunction::PrevPedalboard => "Previous Pedalboard",
            GlobalMidiFunction::ToggleRecording => "Toggle Recording",
            GlobalMidiFunction::ToggleMetronome => "Toggle Metronome"
        };
        write!(f, "{name}")
    }
}

impl GlobalMidiFunction {
    pub fn command_from_function(&self, value: f32) -> Command {
        match self {
            GlobalMidiFunction::ToggleMute => Command::ToggleMute,
            GlobalMidiFunction::SetMasterOut => Command::MasterOut(value),
            GlobalMidiFunction::NextPedalboard => Command::NextPedalboard,
            GlobalMidiFunction::PrevPedalboard => Command::PrevPedalboard,
            GlobalMidiFunction::ToggleRecording => Command::ToggleRecording,
            GlobalMidiFunction::ToggleMetronome => Command::ToggleMetronome,
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