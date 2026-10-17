pub mod biquad;
pub mod eq;
pub mod frequency_analysis;
pub mod impluse_response;
pub mod moving_bandpass;
pub mod oscillator;
pub mod phaser;
pub mod resampler;
pub mod variable_delay;
pub mod variable_delay_phaser;
pub mod yin;

pub fn hann_window(size: usize) -> Vec<f32> {
    let mut window = vec![0.0; size];
    for i in 0..size {
        window[i] = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (size - 1) as f32).cos());
    }
    window
}

pub trait ConvertSampleDbLinear {
    fn db_to_linear(db: Self) -> Self;
    fn linear_to_db(linear: Self) -> Self;
}

impl ConvertSampleDbLinear for f32 {
    fn db_to_linear(db: Self) -> Self {
        10f32.powf(db / 20.0)
    }

    fn linear_to_db(linear: Self) -> Self {
        if linear <= 0.0 {
            f32::NEG_INFINITY
        } else {
            20.0 * linear.log10()
        }
    }
}

impl ConvertSampleDbLinear for f64 {
    fn db_to_linear(db: Self) -> Self {
        10f64.powf(db / 20.0)
    }

    fn linear_to_db(linear: Self) -> Self {
        if linear <= 0.0 {
            f64::NEG_INFINITY
        } else {
            20.0 * linear.log10()
        }
    }
}
