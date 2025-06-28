// Conversion from sample formats to f32
pub fn convert_i8_to_f32(input: &[i8], output: &mut [f32]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (i, &sample) in input.iter().enumerate() {
        output[i] = (sample as f32) / (i8::MAX as f32 + 0.5);
    }
}

pub fn convert_i16_to_f32(input: &[i16], output: &mut [f32]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (i, &sample) in input.iter().enumerate() {
        output[i] = (sample as f32) / (i16::MAX as f32 + 0.5);
    }
}

pub fn convert_i32_to_f32(input: &[i32], output: &mut [f32]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (i, &sample) in input.iter().enumerate() {
        output[i] = (sample as f32) / (i32::MAX as f32 + 0.5);
    }
}

pub fn convert_i64_to_f32(input: &[i64], output: &mut [f32]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (i, &sample) in input.iter().enumerate() {
        output[i] = (sample as f64 / (i64::MAX as f64 + 0.5)) as f32;
    }
}

pub fn convert_u8_to_f32(input: &[u8], output: &mut [f32]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (i, &sample) in input.iter().enumerate() {
        output[i] = (sample as f32 - 128.0) / 128.0;
    }
}

pub fn convert_u16_to_f32(input: &[u16], output: &mut [f32]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (i, &sample) in input.iter().enumerate() {
        output[i] = (sample as f32 - 32768.0) / 32768.0;
    }
}

pub fn convert_u32_to_f32(input: &[u32], output: &mut [f32]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (i, &sample) in input.iter().enumerate() {
        output[i] = (sample as f32 - 2147483648.0) / 2147483648.0;
    }
}

pub fn convert_u64_to_f32(input: &[u64], output: &mut [f32]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (i, &sample) in input.iter().enumerate() {
        output[i] = ((sample as f64 - 9223372036854775808.0) / 9223372036854775808.0) as f32;
    }
}

pub fn convert_f64_to_f32(input: &[f64], output: &mut [f32]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (i, &sample) in input.iter().enumerate() {
        output[i] = sample as f32;
    }
}

// Conversion functions from f32 to other formats
pub fn convert_f32_to_i8(input: &[f32], output: &mut [i8]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (src, dst) in input.iter().zip(output.iter_mut()) {
        let sample = (*src * 127.0).round();
        *dst = sample.clamp(i8::MIN as f32, i8::MAX as f32) as i8;
    }
}

pub fn convert_f32_to_i16(input: &[f32], output: &mut [i16]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (src, dst) in input.iter().zip(output.iter_mut()) {
        let sample = (*src * 32767.0).round();
        *dst = sample.clamp(i16::MIN as f32, i16::MAX as f32) as i16;
    }
}

pub fn convert_f32_to_i32(input: &[f32], output: &mut [i32]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (src, dst) in input.iter().zip(output.iter_mut()) {
        let sample = (*src * 2_147_483_647.0).round();
        *dst = sample.clamp(i32::MIN as f32, i32::MAX as f32) as i32;
    }
}

pub fn convert_f32_to_i64(input: &[f32], output: &mut [i64]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (src, dst) in input.iter().zip(output.iter_mut()) {
        let sample = (*src as f64 * 9_223_372_036_854_775_807.0).round();
        *dst = sample.clamp(i64::MIN as f64, i64::MAX as f64) as i64;
    }
}

pub fn convert_f32_to_u8(input: &[f32], output: &mut [u8]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (src, dst) in input.iter().zip(output.iter_mut()) {
        let sample = (*src * 127.0 + 128.0).round();
        *dst = sample.clamp(u8::MIN as f32, u8::MAX as f32) as u8;
    }
}

pub fn convert_f32_to_u16(input: &[f32], output: &mut [u16]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (src, dst) in input.iter().zip(output.iter_mut()) {
        let sample = (*src * 32_767.0 + 32_768.0).round();
        *dst = sample.clamp(u16::MIN as f32, u16::MAX as f32) as u16;
    }
}

pub fn convert_f32_to_u32(input: &[f32], output: &mut [u32]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (src, dst) in input.iter().zip(output.iter_mut()) {
        let sample = (*src * 2_147_483_647.0 + 2_147_483_648.0).round();
        *dst = sample.clamp(u32::MIN as f32, u32::MAX as f32) as u32;
    }
}

pub fn convert_f32_to_u64(input: &[f32], output: &mut [u64]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (src, dst) in input.iter().zip(output.iter_mut()) {
        let sample = (*src as f64 * 9_223_372_036_854_775_807.0 + 9_223_372_036_854_775_808.0).round();
        *dst = sample.clamp(u64::MIN as f64, u64::MAX as f64) as u64;
    }
}

pub fn convert_f32_to_f32(input: &[f32], output: &mut [f32]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    output.copy_from_slice(input);
}

pub fn convert_f32_to_f64(input: &[f32], output: &mut [f64]) {
    assert_eq!(input.len(), output.len(), "Input and output slices must be of equal length");
    for (src, dst) in input.iter().zip(output.iter_mut()) {
        *dst = *src as f64;
    }
}
