mod args;
mod model;
mod mp3_stream_decoder;
mod player;
mod terminal;
mod update_checker;
mod utils;

use anyhow::{anyhow, Result};
use args::Args;
use clap::Parser;
use colored::Colorize;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use model::{CodeRadioMessage, Remote};
use player::Player;
use prettytable::{cell, row, Table};
use rodio::Source;
use std::{fmt::Write, sync::Mutex, thread, time::Duration};
use terminal::writeline;

const WEBSOCKET_API_URL: &str =
    "wss://coderadio-admin.freecodecamp.org/api/live/nowplaying/coderadio";
const REST_API_URL: &str = "https://coderadio-admin.freecodecamp.org/api/live/nowplaying/coderadio";

static PLAYER: Mutex<Option<Player>> = Mutex::new(None);
static PROGRESS_BAR: Mutex<Option<ProgressBar>> = Mutex::new(None);

#[tokio::main]
async fn main() {
    terminal::enable_color_on_windows();
    let _terminal_clean_up_helper = terminal::create_clean_up_helper(); // See the comments in "terminal" module

    if let Err(e) = start().await {
        writeline!();
        terminal::print_error(e);
    }
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

fn update_status_bar(message: CodeRadioMessage, last_song_id: &mut String){
    // Display song info
    let song = message.now_playing.song;

    let elapsed_seconds = message.now_playing.elapsed;
    let total_seconds = message.now_playing.duration; // Note: This may be 0

    let progress_bar_preffix =
            get_progress_bar_prefix(PLAYER.lock().unwrap().as_ref().map(Player::volume));
    let progress_bar_suffix = get_progress_bar_suffix(message.listeners.current);

    let mut progress_bar_guard = PROGRESS_BAR.lock().unwrap();
    if song.id != *last_song_id {
        if let Some(progress_bar) = progress_bar_guard.as_ref() {
            progress_bar.finish_and_clear();
        }

        *last_song_id = song.id.clone();

        writeline!();
        writeline!("{}       {}", "Song:".bright_green(), song.title);
        writeline!("{}     {}", "Artist:".bright_green(), song.artist);
        writeline!("{}      {}", "Album:".bright_green(), song.album);

        let progress_bar_len = if total_seconds > 0 {
            total_seconds as u64
        } else {
            u64::MAX
        };

        let progress_bar_style = ProgressStyle::with_template("{prefix}  {wide_bar} {progress_info} - {msg}")
            .unwrap()
            .with_key(
                "progress_info",
                |state: &ProgressState, write: &mut dyn Write| {
                    let progress_info =
                        get_progress_bar_progress_info(state.pos(), state.len());
                    write!(write, "{progress_info}").unwrap();
                },
            );

        let progress_bar = ProgressBar::new(progress_bar_len)
            .with_style(progress_bar_style)
            .with_position(elapsed_seconds as u64)
            .with_prefix(progress_bar_preffix)
            .with_message(progress_bar_suffix);
        
        progress_bar.tick();

        *progress_bar_guard = Some(progress_bar);
    } else if let Some(progress_bar) = progress_bar_guard.as_ref() {
            progress_bar.set_position(elapsed_seconds as u64);
            progress_bar.set_message(progress_bar_suffix);
    }
}

async fn start_playing(args: Args) -> Result<()> {
    let mut update_checking_task_holder = Some(tokio::spawn(update_checker::get_new_release()));

    display_welcome_message(&args);

    // Connect websocket in background while creating `Player` to improve startup speed
    let websocket_connect_task = tokio::spawn(tokio_tungstenite::connect_async(WEBSOCKET_API_URL));

    let loading_spinner = ProgressBar::new_spinner()
        .with_style(ProgressStyle::with_template("{spinner} {msg}")?)
        .with_message("Initializing audio device...");
    loading_spinner.enable_steady_tick(Duration::from_millis(120));

    // Creating a `Player` might be time consuming. It might take several seconds on first run.
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

    let mut listen_url = None;
    let mut last_song_id = String::new();

    let (mut websocket_stream, _) = websocket_connect_task.await??;
    tokio::spawn(tick_progress_bar());

    while let Some(message) = parse_websocket_message(websocket_stream.next().await)? {
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
                None => message.station.listen_url.clone(),
            };

            // Notify user if a new version is available
            if let Some(update_checking_task) = update_checking_task_holder.take() {
                if update_checking_task.is_finished() {
                    if let Ok(Ok(Some(new_release))) = update_checking_task.await {
                        writeline!(
                            "{}",
                            format!("New version available: {}", new_release.version)
                                .bright_yellow()
                        );
                        writeline!("{}", new_release.url.bright_yellow());
                        writeline!();
                    }
                }
            }

            if let Some(station) = stations
                .iter()
                .find(|station| station.url == listen_url_value)
            {
                writeline!("{}    {}", "Station:".bright_green(), station.name);
            }

            if let Some(player) = PLAYER.lock().unwrap().as_ref() {
                player.play(&listen_url_value);
            }

            listen_url = Some(listen_url_value);

            thread::spawn(handle_keyboard_events);
        }

        update_status_bar(message, &mut last_song_id);
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
    let stations = get_stations_from_api_message(&message);
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
    stations.sort_by_key(|s| s.id);
    stations
}

fn parse_websocket_message(
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
    let volume_char = volume.map_or_else(|| "*".to_owned(),  |v| v.to_string());
    format!("Volume {volume_char}/9")
}

fn get_progress_bar_suffix(listener_count: i64) -> String {
    format!("Listeners: {listener_count}")
}

// If elapsed seconds and total seconds are both known:
//     "01:14 / 05:14"
//
// If elapsed seconds is known but total seconds is unknown:
//     "01:14"
fn get_progress_bar_progress_info(elapsed_seconds: u64, total_seconds: Option<u64>) -> String {
    let humanized_elapsed_duration =
        utils::humanize_seconds_to_minutes_and_seconds(elapsed_seconds);

    if let Some(total_seconds) = total_seconds {
        if total_seconds != u64::MAX {
            let humanized_total_duration =
                utils::humanize_seconds_to_minutes_and_seconds(total_seconds);
            return format!(
                "{humanized_elapsed_duration} / {humanized_total_duration}"
            );
        }
    }

    humanized_elapsed_duration
}

async fn tick_progress_bar() {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        if let Some(progress_bar) = PROGRESS_BAR.lock().unwrap().as_ref() {
            progress_bar.inc(1);
        }
    }
}

fn handle_keyboard_events() -> ! {
    loop {
        if let Some(n) = terminal::read_char().ok().and_then(|c| c.to_digit(10)) {
            if let Some(player) = PLAYER.lock().unwrap().as_mut() {
                let volume = n as u8;
                if player.volume() == volume {
                    continue;
                }
                player.set_volume(volume);
                if let Some(progress_bar) = PROGRESS_BAR.lock().unwrap().as_mut() {
                    // 波動拳！
                    progress_bar.set_prefix(get_progress_bar_prefix(Some(volume)));
                };
            }
        }
    }
}
