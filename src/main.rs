mod args;
mod model;
mod mp3_stream_decoder;
mod player;
mod terminal;
mod utils;

use anyhow::{anyhow, Result};
use args::Args;
use clap::Parser;
use colored::Colorize;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use model::{CodeRadioMessage, Remote};
use once_cell::sync::Lazy;
use player::Player;
use prettytable::{cell, row, Table};
use rodio::Source;
use std::{sync::Mutex, thread, time::Duration};
use terminal::writeline;

const WEBSOCKET_API_URL: &str =
    "wss://coderadio-admin.freecodecamp.org/api/live/nowplaying/coderadio";
const REST_API_URL: &str = "https://coderadio-admin.freecodecamp.org/api/live/nowplaying/coderadio";

static PLAYER: Lazy<Mutex<Option<Player>>> = Lazy::new(|| Mutex::new(None));
static PROGRESS_BAR: Lazy<Mutex<Option<ProgressBar>>> = Lazy::new(|| Mutex::new(None));

#[tokio::main]
async fn main() {
    if let Err(e) = start().await {
        terminal::print_error(e);
        std::process::exit(1);
    }
}

async fn start() -> Result<()> {
    terminal::enable_color_on_windows();

    let args = Args::parse();

    if args.list_stations {
        print_stations().await?;
    } else {
        if args.volume > 9 {
            return Err(anyhow!("Volume must be between 0 and 9"));
        }

        let result = start_playing(args).await;
        if result.is_err() {
            writeline!();
        }
        result?;
    }

    Ok(())
}

async fn start_playing(args: Args) -> Result<()> {
    display_welcome_message(&args);

    let loading_spinner = ProgressBar::new_spinner()
        .with_style(ProgressStyle::with_template("{spinner} {msg}")?)
        .with_message("Initializing audio device...");
    loading_spinner.enable_steady_tick(Duration::from_millis(120));

    match Player::try_new() {
        Ok(mut player) => {
            player.set_volume(args.volume);
            PLAYER.lock().unwrap().replace(player);
        }
        Err(e) => {
            terminal::print_error(e);
            writeline!();
        }
    }

    loading_spinner.set_message("Connecting...");

    let mut listen_url = Option::None;
    let mut last_song_id = String::new();

    let (mut websocket_stream, _) = tokio_tungstenite::connect_async(WEBSOCKET_API_URL).await?;

    while let Some(message) = parse_websocket_message(websocket_stream.next().await).await? {
        if listen_url.is_none() {
            // Start playing
            loading_spinner.finish_and_clear();

            let stations = get_stations_from_api_message(&message);

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
                writeline!("{}    {}", "Station:".bright_green(), station.name);
            }

            if let Some(player) = &*PLAYER.lock().unwrap() {
                player.play(&listen_url_value);
            }

            listen_url = Some(listen_url_value);

            thread::spawn(handle_keyboard_events);
        }

        // Display song info
        let song = message.now_playing.song;
        let total_seconds = message.now_playing.duration; // Note: This may be 0
        let elapsed_seconds = message.now_playing.elapsed;
        let humanized_total_duration =
            utils::humanize_seconds_to_minutes_and_seconds(total_seconds);
        let humanized_elapsed_duration =
            utils::humanize_seconds_to_minutes_and_seconds(elapsed_seconds);
        let listeners_info = format!("Listeners: {}", message.listeners.current);
        let progress_message = if total_seconds > 0 {
            format!(
                "{} / {} - {}",
                humanized_elapsed_duration, humanized_total_duration, listeners_info
            )
        } else {
            format!("{} - {}", humanized_elapsed_duration, listeners_info)
        };

        let mut progress_bar_guard = PROGRESS_BAR.lock().unwrap();

        if song.id != last_song_id {
            if let Some(progress_bar) = &*progress_bar_guard {
                progress_bar.finish_and_clear();
            }

            last_song_id = song.id.clone();

            writeline!();
            writeline!("{}       {}", "Song:".bright_green(), song.title);
            writeline!("{}     {}", "Artist:".bright_green(), song.artist);
            writeline!("{}      {}", "Album:".bright_green(), song.album);

            let progress_bar_len = if total_seconds > 0 {
                total_seconds as u64
            } else {
                u64::MAX
            };
            let progress_bar = ProgressBar::new(progress_bar_len)
                .with_position(elapsed_seconds as u64)
                .with_message(progress_message)
                .with_style(ProgressStyle::with_template(
                    &(format!("{{prefix}}  {{wide_bar}} {{msg}}")),
                )?);

            let volume = PLAYER.lock().unwrap().as_ref().map(|p| p.volume());
            progress_bar.set_prefix(get_progress_bar_prefix(volume));
            progress_bar.tick();

            *progress_bar_guard = Some(progress_bar);
        } else {
            if let Some(progress_bar) = &*progress_bar_guard {
                progress_bar.set_position(elapsed_seconds as u64);
                progress_bar.set_message(progress_message);
            }
        }
    }

    Ok(())
}

fn display_welcome_message(args: &Args) {
    let logo = "
 ██████╗ ██████╗ ██████╗ ███████╗    ██████╗  █████╗ ██████╗ ██╗ ██████╗ 
██╔════╝██╔═══██╗██╔══██╗██╔════╝    ██╔══██╗██╔══██╗██╔══██╗██║██╔═══██╗
██║     ██║   ██║██║  ██║█████╗      ██████╔╝███████║██║  ██║██║██║   ██║
██║     ██║   ██║██║  ██║██╔══╝      ██╔══██╗██╔══██║██║  ██║██║██║   ██║
╚██████╗╚██████╔╝██████╔╝███████╗    ██║  ██║██║  ██║██████╔╝██║╚██████╔╝
 ╚═════╝ ╚═════╝ ╚═════╝ ╚══════╝    ╚═╝  ╚═╝╚═╝  ╚═╝╚═════╝ ╚═╝ ╚═════╝ ";

    let app_name_and_version = format!("Code Radio CLI v{}", env!("CARGO_PKG_VERSION"));
    let help_command = format!("{} --help", utils::get_current_executable_name());

    let description = format!(
        "{}
A command line music radio client for https://coderadio.freecodecamp.org
GitHub: https://github.com/JasonWei512/code-radio-cli

Press 0-9 to adjust volume. Press Ctrl+C to exit.
Run {} to get more help.",
        app_name_and_version.bright_green(),
        help_command.bright_yellow()
    );

    if !args.no_logo {
        writeline!("{}", logo);
        writeline!();
    }
    writeline!("{}", description);
    writeline!();
}

async fn print_stations() -> Result<()> {
    let stations = get_stations_from_rest_api().await?;

    let mut table = Table::new();
    table.set_format(*prettytable::format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.set_titles(row!["Station ID", "Name", "Bitrate (kbps)"]);
    for station in stations {
        table.add_row(row![station.id, station.name, station.bitrate]);
    }
    table.printstd();

    Ok(())
}

async fn get_stations_from_rest_api() -> Result<Vec<Remote>> {
    let message: CodeRadioMessage = reqwest::get(REST_API_URL).await?.json().await?;
    let mut stations = get_stations_from_api_message(&message);
    stations.sort_by_key(|s| s.id);
    Ok(stations)
}

fn get_stations_from_api_message(message: &CodeRadioMessage) -> Vec<Remote> {
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

fn get_progress_bar_prefix(volume: Option<u8>) -> String {
    let volume_char = match volume {
        Some(v) => v.to_string(),
        None => "*".to_string(),
    };
    format!("Volume {}/9", volume_char)
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
                            progress_bar.set_prefix(get_progress_bar_prefix(Some(volume)));
                            progress_bar.tick();
                        };
                    }
                }
            }
        }
    }
}
