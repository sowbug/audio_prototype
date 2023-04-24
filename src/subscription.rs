use crate::stream::{AudioQueue, AudioStream};
use crossbeam_channel::{unbounded, Receiver, Select, Sender};
use iced::{subscription, Subscription};
use std::fmt::Debug;
use std::time::Instant;
use std::{result::Result::Ok, thread::JoinHandle};

pub enum AudioInterfaceInput {
    SetBufferSize(usize),
    Play,
    Pause,
    Quit,
}

#[derive(Clone, Debug)]
pub enum AudioInterfaceEvent {
    Ready(Sender<AudioInterfaceInput>),
    Reset(usize, AudioQueue),
    NeedsAudio(Instant, usize),
    Quit,
}

enum State {
    Start,
    Ready(
        JoinHandle<()>,                // The AudioStream thread
        Receiver<AudioInterfaceInput>, // App input
        Sender<AudioInterfaceInput>,   // App input forward
        Receiver<AudioInterfaceEvent>, // AudioStream events
    ),
    Ending(JoinHandle<()>),
    Idle,
}

pub struct AudioInterfaceSubscription {}
impl AudioInterfaceSubscription {
    pub fn subscription() -> Subscription<AudioInterfaceEvent> {
        subscription::unfold(
            std::any::TypeId::of::<AudioInterfaceSubscription>(),
            State::Start,
            |state| async move {
                match state {
                    State::Start => {
                        // Sends input from the app to the subscription.
                        let (app_input_sender, app_input_receiver) = unbounded();

                        // Sends events from the audio stream to the subscription.
                        let (audio_stream_event_sender, audio_stream_event_receiver) = unbounded();

                        // Forwards input sent from the app and received by the subscription to the audio-stream thread.
                        let (app_input_forward_sender, app_input_forward_receiver) = unbounded();
                        let handler = std::thread::spawn(move || {
                            if let Ok(mut audio_stream) = AudioStream::create_default_stream(
                                AudioStream::REASONABLE_BUFFER_SIZE,
                                audio_stream_event_sender.clone(),
                            ) {
                                loop {
                                    if let Ok(input) = app_input_forward_receiver.recv() {
                                        match input {
                                            AudioInterfaceInput::SetBufferSize(_) => todo!(),
                                            AudioInterfaceInput::Play => audio_stream.play(),
                                            AudioInterfaceInput::Pause => audio_stream.pause(),
                                            AudioInterfaceInput::Quit => {
                                                audio_stream.quit();
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        });
                        (
                            Some(AudioInterfaceEvent::Ready(app_input_sender)),
                            State::Ready(
                                handler,
                                app_input_receiver,
                                app_input_forward_sender,
                                audio_stream_event_receiver,
                            ),
                        )
                    }
                    State::Ready(
                        handler,
                        app_input_receiver,
                        app_input_forward_sender,
                        audio_stream_event_receiver,
                    ) => {
                        let mut sel = Select::new();
                        sel.recv(&app_input_receiver);
                        sel.recv(&audio_stream_event_receiver);
                        loop {
                            let index = sel.ready();
                            match index {
                                0 => {
                                    let res = app_input_receiver.try_recv();
                                    if let Ok(input) = res {
                                        let _ = app_input_forward_sender.send(input);
                                    }
                                }
                                1 => {
                                    let res = audio_stream_event_receiver.try_recv();
                                    if let Ok(event) = res {
                                        match event {
                                            AudioInterfaceEvent::Quit => {
                                                return (
                                                    Some(AudioInterfaceEvent::Quit),
                                                    State::Ending(handler),
                                                )
                                            }
                                            _ => {
                                                return (
                                                    Some(event),
                                                    State::Ready(
                                                        handler,
                                                        app_input_receiver,
                                                        app_input_forward_sender,
                                                        audio_stream_event_receiver,
                                                    ),
                                                );
                                            }
                                        }
                                    }
                                }
                                _ => panic!(),
                            }
                        }
                    }
                    State::Ending(handler) => {
                        let _ = handler.join();
                        // See https://github.com/iced-rs/iced/issues/1348
                        (None, State::Idle)
                    }
                    State::Idle => {
                        // I took this line from
                        // https://github.com/iced-rs/iced/issues/336, but I
                        // don't understand why it helps. I think it's necessary
                        // for the system to get a chance to process all the
                        // subscription results.
                        let _: () = iced::futures::future::pending().await;
                        (None, State::Idle)
                    }
                }
            },
        )
    }
}
