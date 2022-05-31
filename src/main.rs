mod model;
mod mp3_stream_decoder;
mod player;
mod terminal;
mod utils;

use anyhow::{anyhow, Result};
use clap::Parser;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use model::{CodeRadioMessage, Remote};
use once_cell::sync::Lazy;
use player::Player;
use prettytable::{cell, row, Table};
use rodio::Source;
use std::sync::Mutex;
use terminal::writeline;
use tokio::task::spawn_blocking;
use tokio_tungstenite::connect_async;
use utils::prettify_seconds_to_minutes_and_seconds;

const WEBSOCKET_API_URL: &str =
    "wss://coderadio-admin.freecodecamp.org/api/live/nowplaying/coderadio";
const REST_API_URL: &str = "https://coderadio-admin.freecodecamp.org/api/live/nowplaying/coderadio";

static PLAYER: Lazy<Mutex<Option<Player>>> = Lazy::new(|| Mutex::new(None));
static PROGRESS_BAR: Lazy<Mutex<Option<ProgressBar>>> = Lazy::new(|| Mutex::new(None));

/// A command line music radio client for https://coderadio.freecodecamp.org
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The ID of the station to play from
    #[clap(short, long)]
    station: Option<i64>,

    /// List all stations
    #[clap(short, long)]
    list_stations: bool,

    /// Volume, between 0 and 9
    #[clap(short, long, default_value_t = 9)]
    volume: u8,

    /// Do not display logo
    #[clap(short, long)]
    no_logo: bool,
}

fn main() -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(start());
    runtime.shutdown_background(); // Shutdown without waiting for spawned blocking tasks
    result
}

async fn start() -> Result<()> {
    let args = Args::parse();

    if args.list_stations {
        print_stations().await?;
    } else {
        if args.volume > 9 {
            return Err(anyhow!("Volume must be between 0 and 9"));
        }
        start_playing(args).await?;
    }

    Ok(())
}

async fn start_playing(args: Args) -> Result<()> {
    let logo = "
 ██████╗ ██████╗ ██████╗ ███████╗    ██████╗  █████╗ ██████╗ ██╗ ██████╗ 
██╔════╝██╔═══██╗██╔══██╗██╔════╝    ██╔══██╗██╔══██╗██╔══██╗██║██╔═══██╗
██║     ██║   ██║██║  ██║█████╗      ██████╔╝███████║██║  ██║██║██║   ██║
██║     ██║   ██║██║  ██║██╔══╝      ██╔══██╗██╔══██║██║  ██║██║██║   ██║
╚██████╗╚██████╔╝██████╔╝███████╗    ██║  ██║██║  ██║██████╔╝██║╚██████╔╝
 ╚═════╝ ╚═════╝ ╚═════╝ ╚══════╝    ╚═╝  ╚═╝╚═╝  ╚═╝╚═════╝ ╚═╝ ╚═════╝ ";

    let description = format!(
        "Code Radio CLI v{}
A command line music radio client for https://coderadio.freecodecamp.org
Press 0-9 to adjust volume. Press Ctrl+C to exit.",
        env!("CARGO_PKG_VERSION")
    );

    if !args.no_logo {
        writeline!("{}", logo);
        writeline!();
    }
    writeline!("{}", description);
    writeline!();

    let mut player = Player::try_new()?;
    player.set_volume(args.volume);
    PLAYER.lock().unwrap().replace(player);

    let mut listen_url = Option::None;
    let mut last_song_id = String::new();

    writeline!("Loading... ");

    let (mut ws_stream, _) = connect_async(WEBSOCKET_API_URL).await?;

    while let Some(message) = parse_websocket_message(ws_stream.next().await).await? {
        if listen_url.is_none() {
            // Start playing
            let stations = get_stations(&message);

            let listen_url_value = match args.station {
                Some(station_id) => {
                    match stations.iter().find(|station| station.id == station_id) {
                        Some(station) => station.url.clone(),
                        None => {
                            return Err(anyhow!("Station with ID \"{}\" not found", station_id));
                        }
                    }
                }
                None => message.station.listen_url,
            };

            if let Some(station) = stations
                .iter()
                .find(|station| station.url == listen_url_value)
            {
                writeline!("Station:    {}", station.name);
            }

            if let Some(player) = &*PLAYER.lock().unwrap() {
                player.play(&listen_url_value);
            }

            listen_url = Some(listen_url_value);

            spawn_blocking(handle_keyboard_events);
        }

        // Display song info
        let song = message.now_playing.song;
        let duration = message.now_playing.duration;
        let elapsed = message.now_playing.elapsed;
        let pretty_duration = prettify_seconds_to_minutes_and_seconds(duration);
        let pretty_elapsed = prettify_seconds_to_minutes_and_seconds(elapsed);
        let progress_message = if duration > 0 {
            format!("{} / {}", pretty_elapsed, pretty_duration)
        } else {
            pretty_elapsed
        };

        let mut progress_bar_guard = PROGRESS_BAR.lock().unwrap();

        if song.id != last_song_id {
            if let Some(progress_bar) = &*progress_bar_guard {
                progress_bar.finish_and_clear();
            }

            last_song_id = song.id.clone();

            writeline!();
            writeline!("Song:       {}", song.title);
            writeline!("Artist:     {}", song.artist);
            writeline!("Album:      {}", song.album);

            let progress_bar = if duration > 0 {
                ProgressBar::new(duration as u64)
                    .with_position(elapsed as u64)
                    .with_message(progress_message)
                    .with_style(
                        ProgressStyle::default_bar()
                            .template("Volume {prefix}/9  {wide_bar} {msg}"),
                    )
            } else {
                ProgressBar::new(0)
                    .with_message(progress_message)
                    .with_style(ProgressStyle::default_bar().template("Volume {prefix}/9  {msg}"))
            };

            let volume_string = if let Some(player) = &*PLAYER.lock().unwrap() {
                player.volume().to_string()
            } else {
                "~".to_string()
            };

            progress_bar.set_prefix(volume_string);
            progress_bar.tick();

            *progress_bar_guard = Some(progress_bar);
        } else {
            if let Some(progress_bar) = &*progress_bar_guard {
                progress_bar.set_position(elapsed as u64);
                progress_bar.set_message(progress_message);
            }
        }
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
    let message: CodeRadioMessage = reqwest::get(REST_API_URL).await?.json().await?;
    let mut stations = get_stations(&message);
    stations.sort_by_key(|s| s.id);
    return Ok(stations);
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

fn handle_keyboard_events() -> ! {
    loop {
        if let Some(n) = terminal::read_char().ok().and_then(|c| c.to_digit(10)) {
            if n <= 9 {
                if let Some(player) = PLAYER.lock().unwrap().as_mut() {
                    let volume = n as u8;
                    if player.volume() != volume {
                        player.set_volume(volume);
                        if let Some(progress_bar) = PROGRESS_BAR.lock().unwrap().as_mut() {
                            progress_bar.set_prefix(volume.to_string());
                            progress_bar.tick();
                        };
                    }
                }
            }
        }
    }
}
