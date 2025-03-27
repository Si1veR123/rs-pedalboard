// pub mod psola;
pub mod phase_vocoder;
pub mod delay;
pub mod variable_delay;
pub mod oscillator;
pub mod variable_delay_phaser;

pub fn hann_window(size: usize) -> Vec<f32> {
    let mut window = vec![0.0; size];
    for i in 0..size {
        window[i] = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (size - 1) as f32).cos());
    }
    window
}
