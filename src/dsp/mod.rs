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

pub const MINIMUM_DB: f32 = -70.0;
pub const MAXIMUM_DB_INCREASE: f32 = 40.0; // Used by pedals and settings to limit the maximum volume increase in dB

pub fn hann_window(size: usize) -> Vec<f32> {
    let mut window = vec![0.0; size];
    for i in 0..size {
        window[i] = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (size - 1) as f32).cos());
    }
    window
}

pub trait ConvertSampleDbLinear {
    /// A dB value of `MINIMUM_DB` is mapped to 0.0 (assumed silence)
    fn db_to_linear(db: Self) -> Self;
    fn linear_to_db(linear: Self) -> Self;
    /// Convert to dB and scale to the given range. `MINIMUM_DB`` is mapped to min, 0 dB is mapped to max
    fn linear_to_db_in_range(linear: Self, min: Self, max: Self) -> Self;
}

impl ConvertSampleDbLinear for f32 {
    fn db_to_linear(db: Self) -> Self {
        if db <= MINIMUM_DB {
            0.0
        } else {
            10f32.powf(db / 20.0)
        }
    }

    fn linear_to_db(linear: Self) -> Self {
        if linear <= 0.0 {
            f32::NEG_INFINITY
        } else {
            (20.0 * linear.log10()).max(MINIMUM_DB)
        }
    }

    fn linear_to_db_in_range(linear: Self, min: Self, max: Self) -> Self {
        let db = Self::linear_to_db(linear);
        let clamped_db = db.clamp(MINIMUM_DB, 0.0);
        let normalized_db = (clamped_db - MINIMUM_DB) / -MINIMUM_DB;
        min + normalized_db * (max - min)
    }
}

impl ConvertSampleDbLinear for f64 {
    fn db_to_linear(db: Self) -> Self {
        if db <= MINIMUM_DB as f64 {
            0.0
        } else {
            10f64.powf(db / 20.0)
        }
    }

    fn linear_to_db(linear: Self) -> Self {
        if linear <= 0.0 {
            f64::NEG_INFINITY
        } else {
            (20.0 * linear.log10()).max(MINIMUM_DB as f64)
        }
    }

    fn linear_to_db_in_range(linear: Self, min: Self, max: Self) -> Self {
        let db = Self::linear_to_db(linear);
        let clamped_db = db.clamp(MINIMUM_DB as f64, 0.0);
        let normalized_db = (clamped_db - MINIMUM_DB as f64) / -MINIMUM_DB as f64;
        min + normalized_db * (max - min)
    }
}
