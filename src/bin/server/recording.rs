use std::path::{Path, PathBuf};

use crossbeam::channel::{Receiver, Sender, TryRecvError};
use hound::WavWriter;
use ringbuf::{traits::{Consumer, Observer, Split}, HeapCons, HeapProd};

pub enum RecordingHandleState {
    Active {
        processed_prod: HeapProd<f32>,
        clean_prod: Option<HeapProd<f32>>,
        kill_channel: Sender<()>,
        cons_receiver: Receiver<(HeapCons<f32>, Option<HeapCons<f32>>)>
    },
    Inactive {
        processed: (HeapProd<f32>, HeapCons<f32>),
        clean: Option<(HeapProd<f32>, HeapCons<f32>)>
    },
    Starting,
    Stopping {
        processed_prod: HeapProd<f32>,
        clean_prod: Option<HeapProd<f32>>,
        cons_receiver: Receiver<(HeapCons<f32>, Option<HeapCons<f32>>)>
    },
    Transitioning
}

pub struct RecordingHandle {
    state: RecordingHandleState,
    sample_rate: f32,
    pub output_dir: PathBuf
}

impl RecordingHandle {
    pub fn new(ring_buf_size: usize, output_dir: PathBuf, sample_rate: f32) -> Self {
        let (prod, cons) = ringbuf::HeapRb::<f32>::new(ring_buf_size).split();

        Self {
            state: RecordingHandleState::Inactive {
                processed: (prod, cons),
                clean: None
            },
            output_dir,
            sample_rate
        }
    }

    pub fn set_clean(&mut self, clean: bool) {
        match &mut self.state {
            RecordingHandleState::Inactive { clean: clean_ringbuf, processed: (prod, _) } => {
                if clean && clean_ringbuf.is_none() {
                    let (clean_prod, clean_cons) = ringbuf::HeapRb::<f32>::new(
                        prod.capacity().get()
                    ).split();
                    *clean_ringbuf = Some((clean_prod, clean_cons));
                } else if !clean && clean_ringbuf.is_some() {
                    *clean_ringbuf = None;
                }
            },
            _ => {
                log::warn!("RecordingHandle: Attempted to change clean recording state in invalid state.");
            }
        }
    }

    pub fn is_clean(&self) -> bool {
        match &self.state {
            RecordingHandleState::Inactive { clean, .. } => clean.is_some(),
            RecordingHandleState::Active { clean_prod, .. } => clean_prod.is_some(),
            _ => false
        }
    }

    pub fn tick(&mut self) {
        if matches!(self.state, RecordingHandleState::Stopping { .. }) {
            if let RecordingHandleState::Stopping {
                processed_prod,
                clean_prod,
                cons_receiver
            } = std::mem::replace(&mut self.state, RecordingHandleState::Transitioning) {
                match cons_receiver.try_recv() {
                    Ok((cons, clean_cons)) => {
                        self.state = RecordingHandleState::Inactive {
                            processed: (processed_prod, cons),
                            clean:  match clean_cons {
                                Some(c) => Some((clean_prod.expect("RecordingHandle: Clean producer missing when consumer received."), c)),
                                None => None
                            }
                        };
                    },
                    Err(TryRecvError::Empty) => {
                        self.state = RecordingHandleState::Stopping {
                            processed_prod,
                            clean_prod,
                            cons_receiver
                        };
                    },
                    Err(crossbeam::channel::TryRecvError::Disconnected) => {
                        log::error!("RecordingHandle: Recording thread disconnected unexpectedly.");
                        let (new_prod, new_cons) = ringbuf::HeapRb::<f32>::new(processed_prod.capacity().get()).split();
                        self.state = RecordingHandleState::Inactive {
                            processed: (new_prod, new_cons),
                            clean: clean_prod.map(|p| {
                                let (new_prod, new_cons) = ringbuf::HeapRb::<f32>::new(p.capacity().get()).split();
                                (new_prod, new_cons)
                            })
                        };
                    }
                }
            } else {
                unreachable!();
            }
        }
    }

    pub fn start_recording(&mut self) {
        if !matches!(self.state, RecordingHandleState::Inactive { .. }) {
            log::warn!("RecordingHandle: Attempted to start recording while already active or changing.");
            return;
        }

        if let RecordingHandleState::Inactive { processed, clean } = std::mem::replace(&mut self.state, RecordingHandleState::Starting) {
            let (alive_sender, alive_receiver) = crossbeam::channel::bounded(1);
            let (cons_sender, cons_receiver) = crossbeam::channel::bounded(1);

            let (clean_prod, clean_cons) = match clean {
                Some((p, c)) => (Some(p), Some(c)),
                None => (None, None)
            };

            start_file_writer_thread(
                self.output_dir.clone(),
                processed.1,
                clean_cons,
                self.sample_rate,
                alive_receiver,
                cons_sender
            );
            self.state = RecordingHandleState::Active {
                processed_prod: processed.0,
                clean_prod,
                kill_channel: alive_sender,
                cons_receiver
            };
        } else {
            unreachable!();
        }
    }

    pub fn stop_recording(&mut self) {
        if !matches!(self.state, RecordingHandleState::Active { .. }) {
            log::warn!("RecordingHandle: Attempted to stop recording while not active or changing.");
            return;
        }

        if let RecordingHandleState::Active {
            processed_prod,
            clean_prod,
            kill_channel,
            cons_receiver
        } = std::mem::replace(&mut self.state, RecordingHandleState::Transitioning) {
            // Sending a message will cause the recording thread to finish
            if let Err(e) = kill_channel.send(()) {
                log::error!("RecordingHandle: Failed to send stop signal to recording thread: {}", e);
            }

            self.state = RecordingHandleState::Stopping {
                processed_prod,
                clean_prod,
                cons_receiver
            };
        } else {
            unreachable!();
        }
    }

    pub fn is_recording(&self) -> bool {
        matches!(self.state, RecordingHandleState::Active { .. })
    }

    pub fn recording_producer(&mut self) -> Option<&mut HeapProd<f32>> {
        match &mut self.state {
            RecordingHandleState::Active { processed_prod, .. } => Some(processed_prod),
            RecordingHandleState::Inactive { processed, .. } => Some(&mut processed.0),
            _ => None
        }
    }

    pub fn clean_recording_producer(&mut self) -> Option<&mut HeapProd<f32>> {
        match &mut self.state {
            RecordingHandleState::Active { clean_prod, .. } => clean_prod.as_mut(),
            RecordingHandleState::Inactive { clean, .. } => clean.as_mut().map(|(p, _)| p),
            _ => None
        }
    }
}

pub fn start_file_writer_thread<P: AsRef<Path>>(
    output_dir: P,
    mut reader: HeapCons<f32>,
    clean_reader: Option<HeapCons<f32>>,
    sample_rate: f32,
    alive_channel: crossbeam::channel::Receiver<()>,
    cons_sender: crossbeam::channel::Sender<(HeapCons<f32>, Option<HeapCons<f32>>)>,
) {
    let dir = output_dir.as_ref().to_path_buf();
    std::fs::create_dir_all(&dir).expect("Failed to create recording directory");

    std::thread::spawn(move || {
        log::info!("Starting recording thread, to directory: {:?}", dir);
        let ringbuffer_len = reader.capacity().get();
        // Theoretical time for the ringbuffer to fill up
        let fill_time = ringbuffer_len as f32 / sample_rate;
        // Check the buffer 4 times during that period
        let sleep_time = std::time::Duration::from_secs_f32(fill_time / 4.0);

        let mut file_writer = WavWriter::create(
            dir.join(format!("{}.wav", chrono::Local::now().format("%H%M%S-%d%m%Y"))),
            hound::WavSpec {
                channels: 2,
                sample_rate: sample_rate as u32,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float
            }
        ).expect("Failed to create WAV file");

        log::info!("Recording to file: {:?}", file_writer.spec());

        let mut clean_file_writer = None;
        if let Some(clean_reader) = clean_reader {
            clean_file_writer = Some((
                WavWriter::create(
                dir.join(format!("{}-clean.wav", chrono::Local::now().format("%H%M%S-%d%m%Y"))),
                hound::WavSpec {
                        channels: 2,
                        sample_rate: sample_rate as u32,
                        bits_per_sample: 32,
                        sample_format: hound::SampleFormat::Float
                    }
                ).expect("Failed to create clean WAV file"),
                clean_reader
            ));
        }

        let mut sample_count = 0;

        // If message received, or sender disconnected, stop recording
        while let Err(crossbeam::channel::TryRecvError::Empty) = alive_channel.try_recv() {
            for s in reader.pop_iter() {
                // Output 2 channels
                file_writer.write_sample(s).expect("Failed to write sample to WAV file");
                file_writer.write_sample(s).expect("Failed to write sample to WAV file");
                sample_count += 1;
            }

            if let Some((clean_writer, clean_reader)) = &mut clean_file_writer {
                for s in clean_reader.pop_iter() {
                    // Output 2 channels
                    clean_writer.write_sample(s).expect("Failed to write sample to clean WAV file");
                    clean_writer.write_sample(s).expect("Failed to write sample to clean WAV file");
                }

                if sample_count >= sample_rate as usize {
                    clean_writer.flush().expect("Failed to flush clean WAV file");
                }
            }

            // Flush every second
            if sample_count >= sample_rate as usize {
                file_writer.flush().expect("Failed to flush WAV file");
                sample_count = 0;
            }

            std::thread::sleep(sleep_time);
        }

        log::info!("Stopped recording");

        let (clean_file_writer, clean_reader) = match clean_file_writer {
            Some((w, r)) => (Some(w), Some(r)),
            None => (None, None)
        };

        cons_sender.send((reader, clean_reader)).expect("Failed to send consumer back to main thread");

        file_writer.finalize().expect("Failed to finalize WAV file");
        if let Some(w) = clean_file_writer {
            w.finalize().expect("Failed to finalize clean WAV file");
        }
    });
}
