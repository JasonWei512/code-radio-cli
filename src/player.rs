#![allow(dead_code)]

use anyhow::{Context, Result};
use rodio::{OutputStream, Sink};
use std::sync::mpsc::{self, Sender};
use tokio::task::{spawn_blocking, JoinHandle};

use crate::mp3_stream_decoder::Mp3StreamDecoder;

pub struct Player {
    sender: Sender<PlayerMessage>,
    join_handle: JoinHandle<()>,
    volume: u8, // Between 0 and 10
}

enum PlayerMessage {
    Play { listen_url: String, volume: u8 },
    Volume { volume: u8 },
}

impl Player {
    pub fn new() -> Result<Player> {
        OutputStream::try_default().context("Audio device initialization failed")?;

        let (sender, receiver) = mpsc::channel();
        let join_handle = spawn_blocking(move || {
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
                sink.set_volume(Player::map_volume_to_magnitude(current_volume));

                while let Ok(message) = receiver.recv() {
                    match message {
                        PlayerMessage::Play { listen_url, volume } => {
                            current_listen_url = listen_url;
                            current_volume = volume;
                            break;
                        }
                        PlayerMessage::Volume { volume } => {
                            current_volume = volume;
                            sink.set_volume(Player::map_volume_to_magnitude(current_volume));
                        }
                    }
                }
            }
        });

        Ok(Player {
            sender,
            join_handle,
            volume: 10,
        })
    }

    pub fn play(&mut self, listen_url: &str, volume: u8) {
        self.volume = Self::cap_volume(volume);

        self.sender
            .send(PlayerMessage::Play {
                listen_url: listen_url.to_owned(),
                volume: self.volume,
            })
            .unwrap();
    }

    pub fn volume(&self) -> u8 {
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

    /// Cap the volume to a value between 0 and 10
    fn cap_volume(volume: u8) -> u8 {
        volume.min(10)
    }

    /// Maps a volume between 0 and 10 to a magnitude between 0 and 1.
    fn map_volume_to_magnitude(volume: u8) -> f32 {
        if volume == 0 {
            0.0
        } else {
            0.8_f32.powi((10 - volume) as i32)
        }
    }
}
