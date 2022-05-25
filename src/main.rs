mod model;
mod mp3_stream_decoder;
mod player;
mod stdout_flusher;

use anyhow::{anyhow, Result};
use clap::Parser;
use futures_util::StreamExt;
use model::{CodeRadioMessage, Remote};
use player::Player;
use prettytable::{cell, row, Table};
use rodio::Source;
use stdout_flusher::flush_stdout;
use tokio_tungstenite::connect_async;

const WEBSOCKET_API_URL: &str =
    "wss://coderadio-admin.freecodecamp.org/api/live/nowplaying/coderadio";

/// A command line music client for https://coderadio.freecodecamp.org
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The ID of the station to play from
    #[clap(short, long)]
    station: Option<i64>,

    /// List all stations
    #[clap(short, long)]
    list_stations: bool,

    /// Volume, between 0 and 10
    #[clap(short, long, default_value_t = 10)]
    volume: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    ctrlc::set_handler(|| {
        println!("\n\nGoodbye!");
        std::process::exit(0);
    })?;

    let args = Args::parse();

    if args.list_stations {
        print_stations().await?;
    } else {
        if args.volume > 10 {
            return Err(anyhow!("Volume must be between 0 and 10"));
        }
        start_playing(args.station, args.volume).await?;
    }
    Ok(())
}

async fn start_playing(station_id: Option<i64>, volume: u8) -> Result<()> {
    println!(
        r"
 ██████╗ ██████╗ ██████╗ ███████╗    ██████╗  █████╗ ██████╗ ██╗ ██████╗ 
██╔════╝██╔═══██╗██╔══██╗██╔════╝    ██╔══██╗██╔══██╗██╔══██╗██║██╔═══██╗
██║     ██║   ██║██║  ██║█████╗      ██████╔╝███████║██║  ██║██║██║   ██║
██║     ██║   ██║██║  ██║██╔══╝      ██╔══██╗██╔══██║██║  ██║██║██║   ██║
╚██████╗╚██████╔╝██████╔╝███████╗    ██║  ██║██║  ██║██████╔╝██║╚██████╔╝
 ╚═════╝ ╚═════╝ ╚═════╝ ╚══════╝    ╚═╝  ╚═╝╚═╝  ╚═╝╚═════╝ ╚═╝ ╚═════╝ 

Welcome to Code Radio CLI!
This is a command line music client for https://coderadio.freecodecamp.org
"
    );

    let mut player = Player::new()?;
    let mut listen_url = Option::None;
    let mut last_song_id = String::new();

    println!("Loading... ");

    let (mut ws_stream, _) = connect_async(WEBSOCKET_API_URL).await?;

    while let Some(message) = parse_websocket_message(ws_stream.next().await).await? {
        let stations = get_stations(&message);

        if listen_url.is_none() {
            let listen_url_value = match station_id {
                Some(station_id) => {
                    match stations.iter().find(|station| station.id == station_id) {
                        Some(station) => station.url.clone(),
                        None => {
                            // Bug: This doens't return on Windows
                            // return Err(anyhow!("Station with ID \"{}\" not found", station_id));

                            println!("Error: Station with ID \"{}\" not found", station_id);
                            std::process::exit(1);
                        }
                    }
                }
                None => message.station.listen_url,
            };

            if let Some(station) = stations
                .iter()
                .find(|station| station.url == listen_url_value)
            {
                print!("Station:    {}", station.name);
            }

            player.play(&listen_url_value, volume);
            listen_url = Some(listen_url_value);
        }

        let song = message.now_playing.song;
        if song.id != last_song_id {
            last_song_id = song.id.clone();

            println!();
            println!();
            println!("Song:       {}", song.title);
            println!("Artist:     {}", song.artist);
            println!("Album:      {}", song.album);
        }

        let prettify_seconds = |seconds: i64| format!("{:02}:{:02}", seconds / 60, seconds % 60);

        let duration = message.now_playing.duration;
        let elapsed = message.now_playing.elapsed;
        let pretty_duration = prettify_seconds(duration);
        let pretty_elapsed = prettify_seconds(elapsed);

        if duration > 0 {
            let progress = elapsed as f32 / duration as f32;
            print!(
                "\rProgress:   {} / {} - {:.2}%",
                pretty_elapsed,
                pretty_duration,
                progress * 100.0
            );
        } else {
            print!("\rProgress:   {}", pretty_elapsed);
        }
        flush_stdout();
    }

    Ok(())
}

async fn print_stations() -> Result<()> {
    let stations = get_stations_from_web_api().await?;

    let mut table = Table::new();
    table.set_format(*prettytable::format::consts::FORMAT_NO_LINESEP_WITH_TITLE);

    table.set_titles(row!["Station ID", "Name", "Bitrate (kbps)"]);

    for station in stations {
        table.add_row(row![station.id, station.name, station.bitrate]);
    }

    table.printstd();

    Ok(())
}

async fn get_stations_from_web_api() -> Result<Vec<Remote>> {
    let (mut ws_stream, _) = connect_async(WEBSOCKET_API_URL).await?;

    if let Some(message) = parse_websocket_message(ws_stream.next().await).await? {
        let mut stations = get_stations(&message);
        stations.sort_by_key(|s| s.id);
        return Ok(stations);
    } else {
        return Err(anyhow!("Cannot connect to Code Radio API"));
    }
}

fn get_stations(message: &CodeRadioMessage) -> Vec<Remote> {
    let mut stations: Vec<Remote> = Vec::new();
    for remote in &message.station.remotes {
        stations.push(remote.clone());
    }
    for mount in &message.station.mounts {
        stations.push(mount.clone().into());
    }
    stations
}

async fn parse_websocket_message(
    message: Option<
        Result<tokio_tungstenite::tungstenite::Message, tokio_tungstenite::tungstenite::Error>,
    >,
) -> Result<Option<CodeRadioMessage>> {
    if let Some(message) = message {
        let message: CodeRadioMessage = serde_json::de::from_str(&message?.into_text()?)?;
        Ok(Some(message))
    } else {
        Ok(None)
    }
}
