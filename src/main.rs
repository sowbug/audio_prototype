//! # Audio Prototype #2
//!
//! The purpose of this project is to explore [Iced](https://iced.rs/),
//! [cpal](https://github.com/RustAudio/cpal), and concurrency/parallelism.
//!
//! The project opens an Iced application and a default cpal audio stream, then
//! plays an audio tone at the indicated frequency.
//!
//! [stream::AudioStream] consumes a [crossbeam::queue::ArrayQueue] of
//! [stream::StereoSample]s representing left and right channels.
//! [stream::AudioStream] is available to the app through
//! [AudioInterfaceSubscription] as an Iced subscription. The subscription lets
//! the app know when it needs more data to feed to the audio interface, which
//! the app asks [Synthesizer] to provide through the crossbeam ArrayQueue.
//! 
//! The subscription accepts various input. It can play and pause the stream,
//! which controls whether the audio interface consumes samples from the queue.
//! The app can play and pause [Synthesizer] as well, controlling whether it
//! produces samples for the queue, and it can change the frequency of the
//! synthesized tone.
//! 
//! Now that the interface is nicely encapsulated as a subscription, I'm going
//! to try turning [Synthesizer] into something that demands more computing
//! resources to force the issue of async and/or threading.

use crate::subscription::AudioInterfaceSubscription;
use crossbeam_channel::Sender;
use iced::{
    widget::{Button, Column, Container, Row, Text},
    window, Application, Command, Event, Settings, Subscription, Theme,
};
use iced_aw::Card;
use std::fmt::Debug;
use stream::AudioQueue;
use subscription::{AudioInterfaceEvent, AudioInterfaceInput};
use synthesizer::Synthesizer;

mod stream;
mod subscription;
mod synthesizer;

#[derive(Clone, Debug)]
enum Message {
    AudioInterface(AudioInterfaceEvent),
    Event(iced::Event),
    GeneratorChangeFrequency,
    GeneratorPause,
    GeneratorPlay,
    StreamPause,
    StreamPlay,
}

#[derive(Debug)]
struct AudioPrototype {
    synthesizer: Synthesizer,
    queue: Option<AudioQueue>,
    audio_interface_sender: Option<Sender<AudioInterfaceInput>>,
    sample_rate: Option<usize>,
}
impl Default for AudioPrototype {
    fn default() -> Self {
        Self {
            synthesizer: Synthesizer::new_with(0),
            queue: None,
            audio_interface_sender: None,
            sample_rate: None,
        }
    }
}
impl Application for AudioPrototype {
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (Self::default(), Command::none())
    }

    fn title(&self) -> String {
        "Audio Prototype #1".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::AudioInterface(event) => self.audio_interface_update(event),
            Message::Event(event) => self.handle_system_event(event),
            Message::GeneratorChangeFrequency => self.synthesizer.change_frequency(),
            Message::GeneratorPause => self.synthesizer.pause(),
            Message::GeneratorPlay => self.synthesizer.play(),
            Message::StreamPause => self.audio_interface_pause(),
            Message::StreamPlay => self.audio_interface_play(),
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message, iced::Renderer<Self::Theme>> {
        let synthesizer_card = Card::new(
            Text::new("Synthesizer"),
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
                            self.synthesizer.frequency()
                        ))),
                )
                .push(Button::new(Text::new("Pause")).on_press(Message::GeneratorPause)),
        );
        let queue_len = if let Some(queue) = &self.queue {
            queue.len()
        } else {
            0
        };
        let audio_stream_card = Card::new(
            Text::new("Audio Stream"),
            Column::new()
                .push(Button::new(Text::new("Play")).on_press(Message::StreamPlay))
                .push(Button::new(Text::new("Pause")).on_press(Message::StreamPause))
                .push(Text::new(format!("Queue: {} elements", queue_len))),
        );
        Container::new(Row::new().push(synthesizer_card).push(audio_stream_card)).into()
    }

    fn theme(&self) -> Self::Theme {
        Self::Theme::default()
    }

    fn style(&self) -> <Self::Theme as iced::application::StyleSheet>::Style {
        <Self::Theme as iced::application::StyleSheet>::Style::default()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        Subscription::batch(vec![
            iced_native::subscription::events().map(Message::Event),
            AudioInterfaceSubscription::subscription().map(Message::AudioInterface),
        ])
    }
}
impl AudioPrototype {
    fn audio_interface_update(&mut self, event: AudioInterfaceEvent) {
        match event {
            AudioInterfaceEvent::Ready(sender) => self.audio_interface_sender = Some(sender),
            AudioInterfaceEvent::Reset(sample_rate, queue) => {
                self.sample_rate = Some(sample_rate);
                self.queue = Some(queue);
                self.synthesizer = Synthesizer::new_with(sample_rate);
            }
            AudioInterfaceEvent::NeedsAudio(count) => {
                if let Some(queue) = &self.queue {
                    self.synthesizer.generate_audio(count, queue.clone());
                }
            }
            AudioInterfaceEvent::Quit => {
                // Acknowledged. If we needed to be picky about the shutdown
                // sequence, for example closing one resource only after the
                // audio interface thread ended, then this would be our signal
                // to do that.
            }
        }
    }

    fn audio_interface_play(&self) {
        self.send_to_audio_interface(AudioInterfaceInput::Play);
    }

    fn audio_interface_pause(&self) {
        self.send_to_audio_interface(AudioInterfaceInput::Pause);
    }

    fn send_to_audio_interface(&self, input: AudioInterfaceInput) {
        if let Some(sender) = &self.audio_interface_sender {
            let _ = sender.send(input);
        }
    }

    fn handle_system_event(&mut self, event: Event) {
        if let Event::Window(window::Event::CloseRequested) = event {
            self.send_to_audio_interface(AudioInterfaceInput::Quit);
        }
    }
}

pub fn main() -> iced::Result {
    AudioPrototype::run(Settings {
        exit_on_close_request: false,
        window: window::Settings {
            size: (640, 480),
            ..window::Settings::default()
        },
        ..Settings::default()
    })
}
