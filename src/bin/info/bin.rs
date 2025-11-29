use std::collections::HashMap;

use strum::IntoEnumIterator;

use serde_json;

use rs_pedalboard::pedals::{PedalDiscriminants, PedalParameter, PedalParameterValue, PedalTrait, info::{PedalInfo, ParameterInfo, Info}};

fn main() {
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
                        ParameterInfo::Discrete(vec![
                            PedalParameterValue::Bool(false),
                            PedalParameterValue::Bool(true)
                        ])
                    );
                },
                PedalParameterValue::Int(_) => {
                    pedal_info.parameters.insert(
                        param_name,
                        ParameterInfo::Continuous(param.clone())
                    );
                },
                PedalParameterValue::Float(_) => {
                    pedal_info.parameters.insert(
                        param_name,
                        ParameterInfo::Continuous(param.clone())
                    );
                },
                PedalParameterValue::String(_) => {
                    pedal_info.parameters.insert(
                        param_name,
                        ParameterInfo::Discrete(
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

    let json_str = serde_json::to_string_pretty(&info).expect("Failed to serialize info to JSON");

    match std::env::args().nth(1) {
        Some(path) => std::fs::write(path, json_str).expect("Failed to write info JSON to file"),
        None => println!("{}", json_str),
    }
}
