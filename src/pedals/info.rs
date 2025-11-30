use serde::{Serialize, Deserialize};
use super::{PedalParameter, PedalParameterValue, Oscillator};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub enum ParameterInfo {
    Numerical(PedalParameter),
    Options(Vec<PedalParameterValue>),
    Oscillator(Oscillator)
}

#[derive(Serialize, Deserialize)]
pub struct PedalInfo {
    pub name: String,
    pub parameters: HashMap<String, ParameterInfo>
}

#[derive(Serialize, Deserialize)]
pub struct Info {
    pub pedals: Vec<PedalInfo>
}
