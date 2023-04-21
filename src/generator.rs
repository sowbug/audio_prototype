use crate::stream::{AudioQueue, StereoSample};
use crossbeam::queue::ArrayQueue;
use crossbeam_channel::{unbounded, Receiver, Select, Sender};
use std::fmt::Debug;
use std::result::Result::Ok;

pub enum AudioGeneratorInput {
    /// Start making sound.
    Play,

    /// Stop making sounds.
    Pause,

    /// Change the character of the sound.
    ChangeFrequency,

    /// Generate the recommended number of audio samples and put them in the [AudioQueue].
    GenerateAudio(usize),

    /// End the thread.
    Quit,
}
pub enum AudioGeneratorEvent {
    Frequency(f32),
}

/// [AudioGeneratorWrapper] manages [AudioGenerator] and lets the world
/// communicate with it.
#[derive(Debug)]
pub struct AudioGenerator {
    /// An mpsc channel that reports tone-generator events to the client.
    receiver: Receiver<AudioGeneratorEvent>,

    /// An mpsc channel that let the client send commands to the tone generator.
    sender: Sender<AudioGeneratorInput>,
}
impl AudioGenerator {
    /// Instantiates an [AudioGenerator] with the given sample rate and an
    /// [ArrayQueue] that the audio interface consumes.
    pub fn new_with(
        sample_rate: usize,
        queue: AudioQueue,
        needs_more_data_receiver: Receiver<bool>,
    ) -> Self {
        let (client_sender, generator_receiver) = unbounded();
        let (generator_sender, client_receiver) = unbounded();

        let _handler = std::thread::spawn(move || {
            let mut generator = Synthesizer::new_with(sample_rate);

            // Tell the client the initial frequency.
            Self::send_frequency(&generator_sender, generator.frequency());

            let mut sel = Select::new();
            sel.recv(&generator_receiver);
            sel.recv(&needs_more_data_receiver);
            loop {
                let index = sel.ready();
                match index {
                    0 => {
                        let res = generator_receiver.try_recv();
                        if let Ok(input) = res {
                            match input {
                                AudioGeneratorInput::Play => generator.play(),
                                AudioGeneratorInput::Pause => generator.pause(),
                                AudioGeneratorInput::ChangeFrequency => {
                                    generator.change_frequency();
                                    Self::send_frequency(&generator_sender, generator.frequency());
                                }
                                AudioGeneratorInput::GenerateAudio(sample_count) => {
                                    generator.generate_audio(sample_count, &queue);
                                }
                                AudioGeneratorInput::Quit => break,
                            }
                        }
                    }
                    1 => {
                        let res = needs_more_data_receiver.try_recv();
                        if let Ok(_) = res {
                            let shortfall = queue.capacity() - queue.len();

                            // This is an arbitrary limit so we don't fire up
                            // the audio generator too often if the audio stream
                            // starts asking for very small buffers in a single
                            // callback.
                            if shortfall >= 64 {
                                generator.generate_audio(shortfall, &queue);
                            }
                        }
                    }
                    _ => panic!(),
                }
            }
        });
        Self {
            receiver: client_receiver,
            sender: client_sender,
        }
    }

    pub fn change_frequency(&self) {
        let _ = self.sender.send(AudioGeneratorInput::ChangeFrequency);
    }

    pub fn play(&self) {
        let _ = self.sender.send(AudioGeneratorInput::Play);
    }

    pub fn pause(&self) {
        let _ = self.sender.send(AudioGeneratorInput::Pause);
    }

    fn send(&self, input: AudioGeneratorInput) {
        let _ = self.sender.send(input);
    }

    pub fn receiver(&self) -> &Receiver<AudioGeneratorEvent> {
        &self.receiver
    }

    fn send_frequency(sender: &Sender<AudioGeneratorEvent>, frequency: f32) {
        let _ = sender.send(AudioGeneratorEvent::Frequency(frequency));
    }
}

#[derive(Debug)]
struct Synthesizer {
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

    fn generate_audio(&mut self, count: usize, queue: &ArrayQueue<StereoSample>) {
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
