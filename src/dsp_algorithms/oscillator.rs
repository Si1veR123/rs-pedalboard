use std::iter::Iterator;
use ordered_float::OrderedFloat;
use serde::{Serialize, Deserialize};
use strum_macros::EnumDiscriminants;

#[derive(Clone, Hash, Debug, Serialize, Deserialize, EnumDiscriminants)]
#[serde(tag = "type")]
pub enum Oscillator {
    Sine(Sine),
    Square(Square),
    Sawtooth(Sawtooth),
    Triangle(Triangle)
}

impl Oscillator {
    pub fn get_frequency(&self) -> f32 {
        match self {
            Oscillator::Sine(sine) => sine.frequency.0,
            Oscillator::Square(square) => square.frequency.0,
            Oscillator::Sawtooth(sawtooth) => sawtooth.frequency.0,
            Oscillator::Triangle(triangle) => triangle.frequency.0
        }
    }

    pub fn set_frequency(&mut self, frequency: f32) {
        match self {
            Oscillator::Sine(sine) => sine.frequency = OrderedFloat(frequency),
            Oscillator::Square(square) => square.frequency = OrderedFloat(frequency),
            Oscillator::Sawtooth(sawtooth) => sawtooth.frequency = OrderedFloat(frequency),
            Oscillator::Triangle(triangle) => triangle.frequency = OrderedFloat(frequency)
        }
    }

    pub fn set_phase_offset(&mut self, phase_offset: f32) {
        match self {
            Oscillator::Sine(sine) => sine.phase_offset = OrderedFloat(phase_offset),
            Oscillator::Square(square) => square.phase_offset = OrderedFloat(phase_offset),
            Oscillator::Sawtooth(sawtooth) => sawtooth.phase_offset = OrderedFloat(phase_offset),
            Oscillator::Triangle(triangle) => triangle.phase_offset = OrderedFloat(phase_offset)
        }
    }

    pub fn get_phase_offset(&self) -> f32 {
        match self {
            Oscillator::Sine(sine) => sine.phase_offset.0,
            Oscillator::Square(square) => square.phase_offset.0,
            Oscillator::Sawtooth(sawtooth) => sawtooth.phase_offset.0,
            Oscillator::Triangle(triangle) => triangle.phase_offset.0
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        match self {
            Oscillator::Sine(sine) => sine.sample_rate = OrderedFloat(sample_rate),
            Oscillator::Square(square) => square.sample_rate = OrderedFloat(sample_rate),
            Oscillator::Sawtooth(sawtooth) => sawtooth.sample_rate = OrderedFloat(sample_rate),
            Oscillator::Triangle(triangle) => triangle.sample_rate = OrderedFloat(sample_rate)
        }
    }

    pub fn get_sample_rate(&self) -> f32 {
        match self {
            Oscillator::Sine(sine) => sine.sample_rate.0,
            Oscillator::Square(square) => square.sample_rate.0,
            Oscillator::Sawtooth(sawtooth) => sawtooth.sample_rate.0,
            Oscillator::Triangle(triangle) => triangle.sample_rate.0
        }
    }

    pub fn default(sample_rate: f32) -> Self {
        Oscillator::Sine(Sine::new(sample_rate, 440.0, 0.0, 0.0))
    }
}

impl Iterator for Oscillator {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        match self {
            Oscillator::Sine(sine) => sine.next(),
            Oscillator::Square(square) => square.next(),
            Oscillator::Sawtooth(sawtooth) => sawtooth.next(),
            Oscillator::Triangle(triangle) => triangle.next()
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Hash, Debug)]
pub struct Sine {
    pub sample_rate: OrderedFloat<f32>,
    phase: OrderedFloat<f32>,
    pub phase_offset: OrderedFloat<f32>,
    pub frequency: OrderedFloat<f32>,

    squareness: OrderedFloat<f32>,
    tanh_drive: OrderedFloat<f32>
}

impl Sine {
    pub fn new(sample_rate: f32, frequency: f32, phase_offset: f32, squareness: f32) -> Self {
        Sine {
            sample_rate: OrderedFloat(sample_rate),
            phase: OrderedFloat(0.0),
            phase_offset: OrderedFloat(phase_offset),
            frequency: OrderedFloat(frequency),
            squareness: OrderedFloat(squareness),
            tanh_drive: OrderedFloat(Self::squareness_to_tanh_drive(squareness))
        }
    }

    pub fn get_squareness(&self) -> f32 {
        self.squareness.0
    }

    pub fn set_squareness(&mut self, squareness: f32) {
        self.squareness = OrderedFloat(squareness);
        self.tanh_drive = OrderedFloat(Self::squareness_to_tanh_drive(squareness));
    }

    fn squareness_to_tanh_drive(squareness: f32) -> f32 {
        if squareness == 0.0 {
            return 0.0;
        }

        // Remap 0.0 to 1.0 squareness to 0.0 to 0.99 to prevent log(0)
        let squareness_remapped = squareness.clamp(0.0, 1.0) * 0.99;

        // I made this function by experimenting in desmos, it provides an even, smooth transition from sine to square
        -10.0 * (1.0 - squareness_remapped).log10()
    }

    fn apply_squareness(&self, value: f32) -> f32 {
        (value * self.tanh_drive.0).tanh() / self.tanh_drive.0.tanh()
    }
}

impl Iterator for Sine {
    type Item = f32;

    /// Never returns None
    fn next(&mut self) -> Option<f32> {
        if self.phase.0 > 1.0 {
            self.phase -= 1.0;
        } 

        let value = ((self.phase.0 + self.phase_offset.0) * 2.0 * std::f32::consts::PI).sin();
        self.phase += self.frequency / self.sample_rate;

        if self.squareness.0 == 0.0 {
            Some(value)
        } else {
            Some(self.apply_squareness(value))
        }
    }
}

#[derive(Clone, Hash, Debug, Serialize, Deserialize)]
pub struct Square {
    pub sample_rate: OrderedFloat<f32>,
    phase: OrderedFloat<f32>,
    pub phase_offset: OrderedFloat<f32>,
    pub frequency: OrderedFloat<f32>
}

impl Square {
    pub fn new(sample_rate: f32, frequency: f32, phase_offset: f32) -> Self {
        Square {
            sample_rate: OrderedFloat(sample_rate),
            phase: OrderedFloat(0.0),
            phase_offset: OrderedFloat(phase_offset),
            frequency: OrderedFloat(frequency)
        }
    }
}

impl Iterator for Square {
    type Item = f32;

    /// Never returns None
    fn next(&mut self) -> Option<f32> {
        if self.phase.0 > 1.0 {
            self.phase -= 1.0;
        }

        let value = if (self.phase.0 + self.phase_offset.0) % 1.0 < 0.5 {
            1.0
        } else {
            -1.0
        };
        self.phase += self.frequency / self.sample_rate;
        
        Some(value)
    }
}

#[derive(Clone, Hash, Debug, Serialize, Deserialize)]
pub struct Sawtooth {
    pub sample_rate: OrderedFloat<f32>,
    phase: OrderedFloat<f32>,
    pub phase_offset: OrderedFloat<f32>,
    pub frequency: OrderedFloat<f32>
}

impl Sawtooth {
    pub fn new(sample_rate: f32, frequency: f32, phase_offset: f32) -> Self {
        Sawtooth {
            sample_rate: OrderedFloat(sample_rate),
            phase: OrderedFloat(0.0),
            phase_offset: OrderedFloat(phase_offset),
            frequency: OrderedFloat(frequency)
        }
    }
}

impl Iterator for Sawtooth {
    type Item = f32;

    /// Never returns None
    fn next(&mut self) -> Option<f32> {
        if self.phase.0 > 1.0 {
            self.phase -= 1.0;
        }

        let value = 2.0 * ((self.phase.0 + self.phase_offset.0) % 1.0) - 1.0;
        self.phase += self.frequency / self.sample_rate;
        
        Some(value)
    }
}

#[derive(Clone, Hash, Debug, Serialize, Deserialize)]
pub struct Triangle {
    sample_rate: OrderedFloat<f32>,
    phase: OrderedFloat<f32>,
    pub phase_offset: OrderedFloat<f32>,
    pub frequency: OrderedFloat<f32>
}

impl Triangle {
    pub fn new(sample_rate: f32, frequency: f32, phase_offset: f32) -> Self {
        Triangle {
            sample_rate: OrderedFloat(sample_rate),
            phase: OrderedFloat(0.0),
            phase_offset: OrderedFloat(phase_offset),
            frequency: OrderedFloat(frequency)
        }
    }
}

impl Iterator for Triangle {
    type Item = f32;

    /// Never returns None
    fn next(&mut self) -> Option<f32> {
        if self.phase.0 > 1.0 {
            self.phase -= 1.0;
        }

        let phase_with_offset = (self.phase.0 + self.phase_offset.0) % 1.0;
        let value = if phase_with_offset < 0.5 {
            4.0 * phase_with_offset - 1.0
        } else {
            -4.0 * phase_with_offset + 3.0
        };

        self.phase += self.frequency / self.sample_rate;
        
        Some(value)
    }
}
