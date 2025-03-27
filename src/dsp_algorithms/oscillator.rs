use std::iter::Iterator;

pub enum Oscillator {
    Sine(Sine),
    Square(Square),
    Sawtooth(Sawtooth),
    Triangle(Triangle)
}

impl Oscillator {
    pub fn get_frequency(&self) -> f32 {
        match self {
            Oscillator::Sine(sine) => sine.frequency,
            Oscillator::Square(square) => square.frequency,
            Oscillator::Sawtooth(sawtooth) => sawtooth.frequency,
            Oscillator::Triangle(triangle) => triangle.frequency
        }
    }

    pub fn set_frequency(&mut self, frequency: f32) {
        match self {
            Oscillator::Sine(sine) => sine.frequency = frequency,
            Oscillator::Square(square) => square.frequency = frequency,
            Oscillator::Sawtooth(sawtooth) => sawtooth.frequency = frequency,
            Oscillator::Triangle(triangle) => triangle.frequency = frequency
        }
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


pub struct Sine {
    sample_rate: f32,
    phase: f32,
    pub frequency: f32
}

impl Sine {
    pub fn new(sample_rate: f32, frequency: f32) -> Self {
        Sine {
            sample_rate,
            phase: 0.0,
            frequency
        }
    }
}

impl Iterator for Sine {
    type Item = f32;

    /// Never returns None
    fn next(&mut self) -> Option<f32> {
        let value = (self.phase * 2.0 * std::f32::consts::PI).sin();
        self.phase += self.frequency / self.sample_rate;
        if self.phase > 1.0 {
            self.phase -= 1.0;
        }
        Some(value)
    }
}


pub struct Square {
    sample_rate: f32,
    phase: f32,
    pub frequency: f32
}

impl Square {
    pub fn new(sample_rate: f32, frequency: f32) -> Self {
        Square {
            sample_rate,
            phase: 0.0,
            frequency
        }
    }
}

impl Iterator for Square {
    type Item = f32;

    /// Never returns None
    fn next(&mut self) -> Option<f32> {
        let value = if self.phase < 0.5 {
            1.0
        } else {
            -1.0
        };
        self.phase += self.frequency / self.sample_rate;
        if self.phase > 1.0 {
            self.phase -= 1.0;
        }
        Some(value)
    }
}

pub struct Sawtooth {
    sample_rate: f32,
    phase: f32,
    pub frequency: f32
}

impl Sawtooth {
    pub fn new(sample_rate: f32, frequency: f32) -> Self {
        Sawtooth {
            sample_rate,
            phase: 0.0,
            frequency
        }
    }
}

impl Iterator for Sawtooth {
    type Item = f32;

    /// Never returns None
    fn next(&mut self) -> Option<f32> {
        let value = 2.0 * self.phase - 1.0;
        self.phase += self.frequency / self.sample_rate;
        if self.phase > 1.0 {
            self.phase -= 1.0;
        }
        Some(value)
    }
}

pub struct Triangle {
    sample_rate: f32,
    phase: f32,
    pub frequency: f32
}

impl Triangle {
    pub fn new(sample_rate: f32, frequency: f32) -> Self {
        Triangle {
            sample_rate,
            phase: 0.0,
            frequency
        }
    }
}

impl Iterator for Triangle {
    type Item = f32;

    /// Never returns None
    fn next(&mut self) -> Option<f32> {
        let value = if self.phase < 0.5 {
            4.0 * self.phase - 1.0
        } else {
            -4.0 * self.phase + 3.0
        };
        self.phase += self.frequency / self.sample_rate;
        if self.phase > 1.0 {
            self.phase -= 1.0;
        }
        Some(value)
    }
}
