use anyhow::{Context, Result};
use rodio::{OutputStream, Sink};
use std::sync::mpsc::{self, Sender};
use tokio::task::{spawn_blocking, JoinHandle};

use crate::mp3_stream_decoder::Mp3StreamDecoder;

pub struct Player {
    sender: Sender<PlayerMessage>,
    _playing_handle: JoinHandle<()>,
    volume: u8, // Between 0 and 9
}

enum PlayerMessage {
    Play { listen_url: String, volume: u8 },
    Volume { volume: u8 },
}

impl Player {
    pub fn try_new() -> Result<Player> {
        OutputStream::try_default().context("Audio device initialization failed")?;

        let (sender, receiver) = mpsc::channel();
        let _playing_handle = spawn_blocking(move || {
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
                sink.set_volume(Player::map_volume(current_volume));

                while let Ok(message) = receiver.recv() {
                    match message {
                        PlayerMessage::Play { listen_url, volume } => {
                            current_listen_url = listen_url;
                            current_volume = volume;
                            break;
                        }
                        PlayerMessage::Volume { volume } => {
                            current_volume = volume;
                            sink.set_volume(Player::map_volume(current_volume));
                        }
                    }
                }
            }
        });

        Ok(Player {
            sender,
            _playing_handle,
            volume: 9,
        })
    }

    pub fn play(&self, listen_url: &str) {
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

    /// Cap the volume to a value between 0 and 9
    fn cap_volume(volume: u8) -> u8 {
        volume.min(9)
    }

    /// Maps a volume between 0 and 9 to a magnitude between 0 and 1.
    fn map_volume(volume: u8) -> f32 {
        volume as f32 / 9_f32
    }
}
