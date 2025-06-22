use std::sync::{atomic::AtomicBool, Arc};
use crossbeam::channel::Sender;
use rs_pedalboard::dsp_algorithms::yin::Yin;

pub fn start_tuner(mut yin: Yin, kill: Arc<AtomicBool>, send_to: Sender<f32>) {
    std::thread::spawn(move || {
        log::info!("Tuner thread started");
        let mut consecutive_zeros = 0;

        while !kill.load(std::sync::atomic::Ordering::Relaxed) {
            let frequency = yin.process_buffer();

            if frequency == 0.0 {
                consecutive_zeros += 1;
            } else {
                consecutive_zeros = 0;
            }

            // Only send a freq if the freq is non-zero or if we have had three consecutive zeros.
            // This prevents a single 0 throwing off the tuner, and prevents many consecutive zeros being sent.
            if frequency != 0.0 || consecutive_zeros == 3 {
                if send_to.send(frequency).is_err() {
                    log::error!("Failed to send tuner frequency to audio thread");
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(rs_pedalboard::dsp_algorithms::yin::SERVER_UPDATE_FREQ_MS));
        }
        log::info!("Tuner thread stopped");
    });
}