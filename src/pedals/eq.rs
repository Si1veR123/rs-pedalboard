use std::collections::HashMap;
use super::{PedalParameter, PedalParameterValue, PedalTrait};

use crate::dsp_algorithms::eq;

pub struct GraphicEq7 {
    parameters: HashMap<String, PedalParameter>,
    eq: eq::Equalizer
}

impl GraphicEq7 {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        let init_gains = [0.0; 7];
        let init_bandwidths = [0.0; 7];

        for i in 0..7 {
            parameters.insert(
                format!("gain{}", i + 1),
                PedalParameter {
                    value: PedalParameterValue::Float(init_gains[i]),
                    min: Some(PedalParameterValue::Float(-15.0)),
                    max: Some(PedalParameterValue::Float(15.0)),
                    step: Some(PedalParameterValue::Float(0.1))
                },
            );
            parameters.insert(
                format!("bandwidth{}", i + 1),
                PedalParameter {
                    value: PedalParameterValue::Float(init_bandwidths[i]),
                    min: Some(PedalParameterValue::Float(0.1)),
                    max: Some(PedalParameterValue::Float(2.0)),
                    step: Some(PedalParameterValue::Float(0.01))
                },
            );
        }

        parameters.insert(
            "gain".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(2.0)),
                step: None
            },
        );

        let eq = eq::GraphicEqualizerBuilder::new(48000.0)
            .with_bands([100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0])
            .with_upper_shelf(true)
            .with_bandwidths(init_bandwidths)
            .with_gains(init_gains)
            .build();

        GraphicEq7 { parameters, eq }
    }
}


impl PedalTrait for GraphicEq7 {
    fn process_audio(&mut self, buffer: &mut [f32]) {
        let gain = self.parameters.get("gain").unwrap().value.as_float().unwrap();

        for sample in buffer.iter_mut() {
            *sample = self.eq.process(*sample) * gain;
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    /// TODO: Update eq with parameter changes
    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        if let Some(param) = self.parameters.get_mut(name) {
            if param.is_valid(&value) {
                param.value = value;
            }
        }
    }
}
