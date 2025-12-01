use serde::{Serialize, Deserialize};
use strum::IntoEnumIterator;
use crate::pedals::{PedalDiscriminants, PedalTrait};
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

impl Info {
    pub fn pedal_defaults() -> Self {
        let mut info = Info { pedals: Vec::new() };

        for pedal_type in PedalDiscriminants::iter() {
            let mut init_pedal = pedal_type.new_pedal();

            let mut pedal_info = PedalInfo {
                name: pedal_type.display_name().to_string(),
                parameters: HashMap::new()
            };

            let params: Vec<(String, PedalParameter)> = init_pedal.get_parameters_mut()
                .into_iter()
                .map(|(n, p)| (n.clone(), p.clone()))
                .collect();

            for (name, param) in &params {
                let param_name = name.to_owned();
                
                match param.value.clone() {
                    PedalParameterValue::Bool(_) => {
                        pedal_info.parameters.insert(
                            param_name,
                            ParameterInfo::Options(vec![
                                PedalParameterValue::Bool(false),
                                PedalParameterValue::Bool(true)
                            ])
                        );
                    },
                    PedalParameterValue::Int(_) => {
                        pedal_info.parameters.insert(
                            param_name,
                            ParameterInfo::Numerical(param.clone())
                        );
                    },
                    PedalParameterValue::Float(_) => {
                        pedal_info.parameters.insert(
                            param_name,
                            ParameterInfo::Numerical(param.clone())
                        );
                    },
                    PedalParameterValue::String(_) => {
                        pedal_info.parameters.insert(
                            param_name,
                            ParameterInfo::Options(
                                init_pedal.get_string_values(&name)
                                    .expect("PedalParameterValue::String must have discrete values")
                                    .into_iter()
                                    .map(|s| PedalParameterValue::String(s))
                                    .collect()
                            )
                        );
                    },
                    PedalParameterValue::Oscillator(o) => {
                        pedal_info.parameters.insert(
                            param_name,
                            ParameterInfo::Oscillator(o)
                        );
                    },
                }
            }

            info.pedals.push(pedal_info);
        }

        info
    }
}
