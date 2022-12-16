use anyhow::{Context, Result};
use rodio::{OutputStream, Sink};
use std::{
    sync::mpsc::{self, Sender},
    thread,
};

use crate::mp3_stream_decoder::Mp3StreamDecoder;

pub struct Player {
    sender: Sender<PlayerMessage>,
    volume: u8, // Between 0 and 9
}

enum PlayerMessage {
    Play { listen_url: String, volume: u8 },
    Volume { volume: u8 },
}

impl Player {
    /// Creating a `Player` might be time consuming. It might take several seconds on first run.
    pub fn try_new() -> Result<Self> {
        OutputStream::try_default().context("Audio device initialization failed")?;

        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let (_stream, stream_handle) = OutputStream::try_default().unwrap();

            let (mut current_listen_url, mut current_volume) = loop {
                if let Ok(PlayerMessage::Play { listen_url, volume }) = receiver.recv() {
                    break (listen_url, volume);
                }
            };

            loop {
                let response = reqwest::blocking::get(&current_listen_url).unwrap();
                let source = Mp3StreamDecoder::new(response).unwrap();
                let sink = Sink::try_new(&stream_handle).unwrap();
                sink.append(source);
                sink.set_volume(Self::map_volume_to_rodio_volume(current_volume));

                while let Ok(message) = receiver.recv() {
                    match message {
                        PlayerMessage::Play { listen_url, volume } => {
                            current_listen_url = listen_url;
                            current_volume = volume;
                            break;
                        }
                        PlayerMessage::Volume { volume } => {
                            current_volume = volume;
                            sink.set_volume(Self::map_volume_to_rodio_volume(current_volume));
                        }
                    }
                }
            }
        });

        Ok(Self { sender, volume: 9 })
    }

    pub fn play(&self, listen_url: &str) {
        self.sender
            .send(PlayerMessage::Play {
                listen_url: listen_url.to_owned(),
                volume: self.volume,
            })
            .unwrap();
    }

    pub const fn volume(&self) -> u8 {
        self.volume
    }

    pub fn set_volume(&mut self, volume: u8) {
        self.volume = Self::cap_volume(volume);

        self.sender
            .send(PlayerMessage::Volume {
                volume: self.volume,
            })
            .unwrap();
    }

    /// Cap volume to a value between 0 and 9
    fn cap_volume(volume: u8) -> u8 {
        volume.min(9)
    }

    /// Map a volume between 0 and 9 to between 0 and 1
    fn map_volume_to_rodio_volume(volume: u8) -> f32 {
        volume as f32 / 9_f32
    }
}
