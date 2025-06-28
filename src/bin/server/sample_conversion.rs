pub struct SampleConverter {
    buffer: Vec<f32>,
}

impl SampleConverter {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
        }
    }

    pub fn convert_i8(&mut self, input: &[i8]) {
        self.buffer.resize(input.len(), 0.0);
        for (i, &sample) in input.iter().enumerate() {
            self.buffer[i] = (sample as f32) / (i8::MAX as f32 + 0.5);
        }
    }

    pub fn convert_i16(&mut self, input: &[i16]) {
        self.buffer.resize(input.len(), 0.0);
        for (i, &sample) in input.iter().enumerate() {
            self.buffer[i] = (sample as f32) / (i16::MAX as f32 + 0.5);
        }
    }

    pub fn convert_i32(&mut self, input: &[i32]) {
        self.buffer.resize(input.len(), 0.0);
        for (i, &sample) in input.iter().enumerate() {
            self.buffer[i] = (sample as f32) / (i32::MAX as f32 + 0.5);
        }
    }

    pub fn convert_i64(&mut self, input: &[i64]) {
        self.buffer.resize(input.len(), 0.0);
        for (i, &sample) in input.iter().enumerate() {
            self.buffer[i] = (sample as f64 / (i64::MAX as f64 + 0.5)) as f32;
        }
    }

    pub fn convert_u8(&mut self, input: &[u8]) {
        self.buffer.resize(input.len(), 0.0);
        for (i, &sample) in input.iter().enumerate() {
            self.buffer[i] = (sample as f32 - 128.0) / 128.0;
        }
    }

    pub fn convert_u16(&mut self, input: &[u16]) {
        self.buffer.resize(input.len(), 0.0);
        for (i, &sample) in input.iter().enumerate() {
            self.buffer[i] = (sample as f32 - 32768.0) / 32768.0;
        }
    }

    pub fn convert_u32(&mut self, input: &[u32]) {
        self.buffer.resize(input.len(), 0.0);
        for (i, &sample) in input.iter().enumerate() {
            self.buffer[i] = (sample as f32 - 2147483648.0) / 2147483648.0;
        }
    }

    pub fn convert_u64(&mut self, input: &[u64]) {
        self.buffer.resize(input.len(), 0.0);
        for (i, &sample) in input.iter().enumerate() {
            self.buffer[i] = ((sample as f64 - 9223372036854775808.0) / 9223372036854775808.0) as f32;
        }
    }

    pub fn convert_f64(&mut self, input: &[f64]) {
        self.buffer.resize(input.len(), 0.0);
        for (i, &sample) in input.iter().enumerate() {
            self.buffer[i] = sample as f32;
        }
    }
}

impl AsRef<[f32]> for SampleConverter {
    fn as_ref(&self) -> &[f32] {
        &self.buffer
    }
}

impl AsMut<[f32]> for SampleConverter {
    fn as_mut(&mut self) -> &mut [f32] {
        &mut self.buffer
    }
}
