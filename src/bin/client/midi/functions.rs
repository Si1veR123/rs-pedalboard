use rs_pedalboard::pedals::PedalParameterValue;
use serde::{Serialize, Deserialize};

use crate::socket::{Command, ParameterPath};

// Functions that can be triggered by MIDI devices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MidiFunctions {
    Global(Vec<GlobalMidiFunction>),
    Parameter(Vec<ParameterMidiFunction>)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GlobalMidiFunction {
    Mute,
    SetMasterOut,
    NextPedalboard,
    PrevPedalboard,
    ToggleRecording,
    ToggleMetronome
}

impl GlobalMidiFunction {
    pub fn command_from_function(&self, value: f32) -> Option<Command> {
        match self {
            GlobalMidiFunction::Mute => Some(Command::SetMute(value >= 0.5)),
            GlobalMidiFunction::SetMasterOut => Some(Command::MasterOut(value)),
            GlobalMidiFunction::NextPedalboard => {
                if value >= 0.5 { Some(Command::NextPedalboard) } else { None }
            },
            GlobalMidiFunction::PrevPedalboard => {
                if value >= 0.5 { Some(Command::PrevPedalboard) } else { None }
            },
            GlobalMidiFunction::ToggleRecording => {
                if value >= 0.5 { Some(Command::ToggleRecording) } else { None }
            },
            GlobalMidiFunction::ToggleMetronome => {
                if value >= 0.5 { Some(Command::ToggleMetronome) } else { None }
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterMidiFunction {
    pub pedalboard_id: u32,
    pub pedal_id: u32,
    pub parameter_name: String,
    pub min_value: PedalParameterValue,
    pub max_value: PedalParameterValue
}

impl ParameterMidiFunction {
    fn parameter_from_value(&self, value: f32) -> PedalParameterValue {
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

    pub fn command_from_value(&self, value: f32) -> Command {
        Command::ParameterUpdate(ParameterPath {
            pedalboard_id: self.pedalboard_id,
            pedal_id: self.pedal_id,
            parameter_name: self.parameter_name.clone()
        }, self.parameter_from_value(value))
    }
}