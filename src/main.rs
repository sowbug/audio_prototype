//! # Audio Prototype #1
//!
//! The purpose of this project is to explore [Iced](https://iced.rs/),
//! [cpal](https://github.com/RustAudio/cpal), and concurrency/parallelism.
//!
//! Right now, the project opens an Iced application and a default cpal audio
//! stream, then plays an audio tone at the indicated frequency.
//!
//! [AudioStream] consumes a [crossbeam::queue::ArrayQueue] of
//! [stream::StereoSample]s representing left and right channels.
//! [AudioGenerator] produces samples.
//!
//! Other than the queue, [AudioStream] and [AudioGenerator] communicate through
//! a crossbeam channel. As [AudioStream] consumes a batch of samples, it sends
//! a single boolean over the channel, indicating that [AudioGenerator] should
//! fill the queue with more samples.
//!
//! The Iced app is [AudioPrototype]. It owns both [AudioStream] and
//! [AudioGenerator]. [AudioStream] can play and pause (controlling whether it
//! consumes samples from the queue). [AudioGenerator] can play and pause as
//! well (controlling whether it produces samples for the queue), and it can
//! change the frequency of the synthesized tone.
//! 
//! Next: I'll explore turning [AudioStream] into an Iced subscription.

use crossbeam_channel::bounded;
use generator::AudioGenerator;
use iced::{
    widget::{Button, Column, Container, Row, Text},
    window, Application, Command, Settings, Theme,
};
use iced_aw::Card;
use std::{fmt::Debug, sync::Arc};
use std::{result::Result::Ok, time::Instant};
use stream::AudioStream;

mod generator;
mod stream;

#[derive(Debug, Clone)]
enum Message {
    Tick(Instant),
    StreamPlay,
    StreamPause,
    GeneratorPlay,
    GeneratorPause,
    GeneratorChangeFrequency,
}

#[derive(Debug)]
struct AudioPrototype {
    audio_generator: AudioGenerator,
    audio_stream: AudioStream,
    last_known_frequency: f32,
}
impl Default for AudioPrototype {
    fn default() -> Self {
        let (s, r) = bounded(1);
        if let Ok(audio_stream) =
            AudioStream::create_default_stream(AudioStream::REASONABLE_BUFFER_SIZE, s)
        {
            Self {
                audio_generator: AudioGenerator::new_with(
                    audio_stream.sample_rate(),
                    Arc::clone(&audio_stream.queue()),
                    r,
                ),
                audio_stream,
                last_known_frequency: 0.0,
            }
        } else {
            panic!()
        }
    }
}
impl Application for AudioPrototype {
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (Self::new_with(), Command::none())
    }

    fn title(&self) -> String {
        "Audio Prototype #1".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Tick(_when) => {
                if let Ok(event) = self.audio_generator.receiver().try_recv() {
                    match event {
                        generator::AudioGeneratorEvent::Frequency(f) => {
                            self.last_known_frequency = f
                        }
                    }
                }
            }
            Message::StreamPlay => self.audio_stream.play(),
            Message::StreamPause => self.audio_stream.pause(),
            Message::GeneratorChangeFrequency => self.audio_generator.change_frequency(),
            Message::GeneratorPlay => self.audio_generator.play(),
            Message::GeneratorPause => self.audio_generator.pause(),
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message, iced::Renderer<Self::Theme>> {
        let audio_generator_card = Card::new(
            Text::new("Audio Generator"),
            Column::new()
                .push(Button::new(Text::new("Play")).on_press(Message::GeneratorPlay))
                .push(
                    Row::new()
                        .push(
                            Button::new(Text::new("Change Frequency"))
                                .on_press(Message::GeneratorChangeFrequency),
                        )
                        .push(Text::new(format!(
                            "Frequency: {:0.2} Hz",
                            self.last_known_frequency
                        ))),
                )
                .push(Button::new(Text::new("Pause")).on_press(Message::GeneratorPause)),
        );
        let queue_len = self.audio_stream.queue().len();
        let audio_stream_card = Card::new(
            Text::new("Audio Stream"),
            Column::new()
                .push(Button::new(Text::new("Play")).on_press(Message::StreamPlay))
                .push(Button::new(Text::new("Pause")).on_press(Message::StreamPause))
                .push(Text::new(format!("Queue: {} elements", queue_len))),
        );
        Container::new(
            Row::new()
                .push(audio_generator_card)
                .push(audio_stream_card),
        )
        .into()
    }

    fn theme(&self) -> Self::Theme {
        Self::Theme::default()
    }

    fn style(&self) -> <Self::Theme as iced::application::StyleSheet>::Style {
        <Self::Theme as iced::application::StyleSheet>::Style::default()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        window::frames().map(Message::Tick)
    }
}
impl AudioPrototype {
    fn new_with() -> Self {
        Self::default()
    }
}

pub fn main() -> iced::Result {
    AudioPrototype::run(Settings {
        window: window::Settings {
            size: (1024, 768),
            ..window::Settings::default()
        },
        ..Settings::default()
    })
}
