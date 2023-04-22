use crate::stream::{AudioQueue, StereoSample};
use std::fmt::Debug;

#[derive(Debug)]
pub struct Synthesizer {
    pub sample_rate: usize,
    pub sample_clock: usize,

    pub frequency: f32,
    is_playing: bool,
}

impl Synthesizer {
    pub fn new_with(sample_rate: usize) -> Self {
        Self {
            sample_rate,
            sample_clock: 0,
            frequency: 440.0,
            is_playing: true,
        }
    }

    pub fn play(&mut self) {
        self.is_playing = true;
    }

    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    pub fn change_frequency(&mut self) {
        self.frequency = self.frequency * 1.01;
    }

    pub fn frequency(&self) -> f32 {
        self.frequency
    }

    fn make_sound(&self, freq: f32) -> f32 {
        (self.sample_clock as f32 * freq * 2.0 * std::f32::consts::PI / self.sample_rate as f32)
            .sin()
    }

    fn tick(&mut self) {
        self.sample_clock = self.sample_clock + 1;
    }

    pub fn generate_audio(&mut self, count: usize, queue: AudioQueue) {
        for _ in 0..count {
            let sample = if self.is_playing {
                self.make_sound(self.frequency)
            } else {
                0.0
            };

            // Normally we'd produce and push empty samples even if we weren't
            // playing, but for this demo it's more useful to let the queue
            // shrink.
            if self.is_playing {
                let stereo_sample = StereoSample {
                    left: sample,
                    right: sample,
                };
                let _ = queue.force_push(stereo_sample);
            }
            self.tick();
        }
    }
}
