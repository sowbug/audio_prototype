use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, FromSample, Host, Sample, SizedSample, Stream, SupportedStreamConfig,
};
use crossbeam::queue::ArrayQueue;
use crossbeam_channel::Sender;
use std::result::Result::Ok;
use std::{fmt::Debug, sync::Arc};

#[derive(Debug, Default)]
pub struct StereoSample {
    pub left: f32,
    pub right: f32,
}

pub type AudioQueue = Arc<ArrayQueue<StereoSample>>;

/// Describes the audio interface.
struct StreamInfo {
    #[allow(dead_code)]
    host: Host,
    #[allow(dead_code)]
    device: Device,
    config: SupportedStreamConfig,
}
impl Debug for StreamInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioStream")
            .field("host", &"(skipped)")
            .field("device", &"(skipped)")
            .field("config", &self.config)
            .finish()
    }
}

/// Encapsulates the connection to the audio interface.
pub struct AudioStream {
    stream_info: StreamInfo,
    stream: Stream,
    queue: AudioQueue,
}
impl Debug for AudioStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioStream")
            .field("stream_info", &self.stream_info)
            .field("stream", &"(skipped)")
            .field("queue", &self.queue)
            .finish()
    }
}
impl AudioStream {
    /// This constant is provided to prevent decision paralysis when picking a
    /// `buffer_size` argument. At a typical sample rate of 44.1KHz, a value of
    /// 2048 would mean that samples at the end of a full buffer wouldn't reach
    /// the audio interface for 46.44 milliseconds, which is arguably not
    /// reasonable because audio latency is perceptible at as few as 10
    /// milliseconds. However, on my Ubuntu 20.04 machine, the audio interface
    /// asks for around 2,600 samples (1,300 stereo samples) at once, which
    /// means that 2,048 leaves a cushion of less than a single callback of
    /// samples.
    pub const REASONABLE_BUFFER_SIZE: usize = 2048;

    pub fn create_default_stream(
        buffer_size: usize,
        needs_more_data_sender: Sender<bool>,
    ) -> Result<Self, ()> {
        if let Ok((host, device, config)) = Self::host_device_setup() {
            let queue = Arc::new(ArrayQueue::new(buffer_size));
            if let Ok(stream) = Self::stream_setup_for(
                &device,
                &config,
                &Arc::clone(&queue),
                needs_more_data_sender,
            ) {
                let r = Self {
                    stream_info: StreamInfo {
                        host,
                        device,
                        config,
                    },
                    stream,
                    queue,
                };
                Ok(r)
            } else {
                Err(())
            }
        } else {
            Err(())
        }
    }

    /// Returns the sample rate of the current audio stream.
    pub fn sample_rate(&self) -> usize {
        let config: &cpal::StreamConfig = &self.stream_info.config.clone().into();
        config.sample_rate.0 as usize
    }

    /// Returns the ArrayQueue<f32> that the audio stream consumes. Each item is
    /// a sample for one channel. For stereo streams, the channels are
    /// interleaved. That means that the first f32 is for the left channel of
    /// sample #0, the second f32 is for the right channel of sample #0, and so
    /// on.
    pub fn queue(&self) -> &AudioQueue {
        &self.queue
    }

    /// Tells the audio stream to stop playing audio (which means it will also
    /// stop consuming samples from the queue).
    pub fn play(&self) {
        let _ = self.stream.play();
    }

    /// Tells the audio stream to resume playing audio (and consuming samples
    /// from the queue).
    pub fn pause(&self) {
        let _ = self.stream.pause();
    }

    /// Returns the default host, device, and stream config (all of which are
    /// cpal concepts).
    fn host_device_setup(
    ) -> anyhow::Result<(cpal::Host, cpal::Device, cpal::SupportedStreamConfig), anyhow::Error>
    {
        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::Error::msg("Default output device is not available"))?;
        println!("Output device : {}", device.name()?);

        let config = device.default_output_config()?;
        println!("Default output config : {:?}", config);

        Ok((host, device, config))
    }

    /// Creates and returns a Stream for the given device and config. The Stream
    /// will consume the supplied ArrayQueue<f32>. This function is actually a
    /// wrapper around the generic stream_make<T>().
    fn stream_setup_for(
        device: &cpal::Device,
        config: &SupportedStreamConfig,
        queue: &AudioQueue,
        needs_more_data_sender: Sender<bool>,
    ) -> anyhow::Result<Stream, anyhow::Error> {
        let config = config.clone();

        match config.sample_format() {
            cpal::SampleFormat::I8 => todo!(),
            cpal::SampleFormat::I16 => todo!(),
            cpal::SampleFormat::I32 => todo!(),
            cpal::SampleFormat::I64 => todo!(),
            cpal::SampleFormat::U8 => todo!(),
            cpal::SampleFormat::U16 => todo!(),
            cpal::SampleFormat::U32 => todo!(),
            cpal::SampleFormat::U64 => todo!(),
            cpal::SampleFormat::F32 => {
                Self::stream_make::<f32>(&config.into(), &device, queue, needs_more_data_sender)
            }
            cpal::SampleFormat::F64 => todo!(),
            _ => todo!(),
        }
    }

    /// Generic portion of stream_setup_for().
    fn stream_make<T>(
        config: &cpal::StreamConfig,
        device: &cpal::Device,
        queue: &AudioQueue,
        needs_more_data_sender: Sender<bool>,
    ) -> Result<Stream, anyhow::Error>
    where
        T: SizedSample + FromSample<f32>,
    {
        let err_fn = |err| eprintln!("Error building output sound stream: {}", err);

        let queue = Arc::clone(&queue);
        let channel_count = config.channels as usize;
        let stream = device.build_output_stream(
            config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                Self::on_window(
                    output,
                    channel_count,
                    &queue,
                    needs_more_data_sender.clone(),
                )
            },
            err_fn,
            None,
        )?;
        Ok(stream)
    }

    /// cpal callback that supplies samples from the ArrayQueue<f32>, converting
    /// them if needed to the stream's expected data type.
    fn on_window<T>(
        output: &mut [T],
        channel_count: usize,
        queue: &AudioQueue,
        needs_more_data_sender: Sender<bool>,
    ) where
        T: Sample + FromSample<f32>,
    {
        eprintln!("output length {}", output.len());
        for frame in output.chunks_exact_mut(channel_count) {
            let sample = queue.pop().unwrap_or_default();
            frame[0] = T::from_sample(sample.left);
            if channel_count > 0 {
                frame[1] = T::from_sample(sample.right);
            }
        }
        let _ = needs_more_data_sender.send(true);
    }
}
