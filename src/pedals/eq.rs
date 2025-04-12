use std::collections::HashMap;
use std::hash::Hash;

use serde::{Deserialize, Serialize};

use super::{PedalParameter, PedalParameterValue, PedalTrait};
use super::ui::pedal_knob;

use crate::dsp_algorithms::eq::{self, Equalizer};

#[derive(Clone)]
pub struct GraphicEq7 {
    parameters: HashMap<String, PedalParameter>,
    eq: eq::Equalizer
}

impl Hash for GraphicEq7 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl Serialize for GraphicEq7 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_map(self.parameters.iter())
    }
}

impl<'a> Deserialize<'a> for GraphicEq7 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let parameters: HashMap<String, PedalParameter> = HashMap::deserialize(deserializer)?;
        
        let eq = Equalizer::new(vec![]);

        let pedal = GraphicEq7 {
            parameters,
            eq
        };

        let actual_eq = Self::build_eq(pedal.get_bandwidths(), pedal.get_gains());
        Ok(GraphicEq7 {
            parameters: pedal.parameters,
            eq: actual_eq
        })
    }
}


impl GraphicEq7 {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        let init_gains = [0.0; 7];
        let init_bandwidths = [0.5; 7];

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
                max: Some(PedalParameterValue::Float(4.0)),
                step: None
            },
        );

        let eq = Self::build_eq(init_bandwidths, init_gains);

        GraphicEq7 { parameters, eq }
    }

    pub fn get_gains(&self) -> [f32; 7] {
        [
            self.parameters.get("gain1").unwrap().value.as_float().unwrap(),
            self.parameters.get("gain2").unwrap().value.as_float().unwrap(),
            self.parameters.get("gain3").unwrap().value.as_float().unwrap(),
            self.parameters.get("gain4").unwrap().value.as_float().unwrap(),
            self.parameters.get("gain5").unwrap().value.as_float().unwrap(),
            self.parameters.get("gain6").unwrap().value.as_float().unwrap(),
            self.parameters.get("gain7").unwrap().value.as_float().unwrap(),
        ]
    }

    pub fn get_bandwidths(&self) -> [f32; 7] {
        [
            self.parameters.get("bandwidth1").unwrap().value.as_float().unwrap(),
            self.parameters.get("bandwidth2").unwrap().value.as_float().unwrap(),
            self.parameters.get("bandwidth3").unwrap().value.as_float().unwrap(),
            self.parameters.get("bandwidth4").unwrap().value.as_float().unwrap(),
            self.parameters.get("bandwidth5").unwrap().value.as_float().unwrap(),
            self.parameters.get("bandwidth6").unwrap().value.as_float().unwrap(),
            self.parameters.get("bandwidth7").unwrap().value.as_float().unwrap(),
        ]
    }

    fn build_eq(bandwidths: [f32; 7], gains: [f32; 7]) -> eq::Equalizer {
        eq::GraphicEqualizerBuilder::new(48000.0)
            .with_bands([100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0])
            .with_upper_shelf()
            .with_bandwidths(bandwidths)
            .with_gains(gains)
            .build()
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

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        if let Some(param) = self.parameters.get_mut(name) {
            if param.is_valid(&value) {
                param.value = value;

                if name.starts_with("gain") || name.starts_with("bandwidth") {
                    let gains = self.get_gains();
                    let bandwidths = self.get_bandwidths();
                    self.eq = Self::build_eq(bandwidths, gains);
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui) -> Option<(String, PedalParameterValue)> {
        //let mut to_change = None;
        //for (parameter_name, parameter) in self.get_parameters().iter() {
        //    if let Some(value) = pedal_knob(ui, parameter_name, parameter) {
        //        to_change = Some((parameter_name.clone(), value));
        //    }
        //}
//
        //to_change
        None
    }
}
