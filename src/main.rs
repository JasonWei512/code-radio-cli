mod args;
mod model;
mod mp3_stream_decoder;
mod player;
mod terminal;
mod update_checker;
mod utils;

use anyhow::{anyhow, Context, Result};
use args::Args;
use clap::Parser;
use colored::Colorize;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use inquire::Select;
use model::{CodeRadioMessage, Remote};
use player::Player;
use rodio::Source;
use std::{fmt::Write, sync::Mutex, thread, time::Duration};
use tokio::{net::TcpStream, time::sleep};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

const WEBSOCKET_API_URL: &str =
    "wss://coderadio-admin.freecodecamp.org/api/live/nowplaying/coderadio";
const REST_API_URL: &str = "https://coderadio-admin.freecodecamp.org/api/live/nowplaying/coderadio";

const LOADING_SPINNER_TICK_INTERVAL: Duration = Duration::from_millis(120);

static PLAYER: Mutex<Option<Player>> = Mutex::new(None);
static PROGRESS_BAR: Mutex<Option<ProgressBar>> = Mutex::new(None);

#[tokio::main]
async fn main() {
    terminal::enable_color_on_windows();
    let _terminal_clean_up_helper = terminal::create_clean_up_helper(); // See the comments in "terminal" module

    if let Err(e) = start().await {
        println!();
        terminal::print_error(e);
    }
}

async fn start() -> Result<()> {
    let args = Args::parse();

    if args.volume > 9 {
        return Err(anyhow!("Volume must be between 0 and 9"));
    }

    start_playing(args).await?;

    Ok(())
}

async fn start_playing(args: Args) -> Result<()> {
    // Check update in background
    let update_checking_task = tokio::spawn(update_checker::get_new_release());

    display_welcome_message(&args);

    let selected_station: Option<Remote> = if args.select_station {
        let station = select_station_interactively().await?;
        Some(station)
    } else {
        None
    };

    // Connect WebSocket in background while creating `Player` to improve startup speed
    let websocket_connect_task = tokio::spawn(tokio_tungstenite::connect_async(WEBSOCKET_API_URL));

    let loading_spinner = ProgressBar::new_spinner()
        .with_style(ProgressStyle::with_template("{spinner} {msg}")?)
        .with_message("Initializing audio device...");
    loading_spinner.enable_steady_tick(LOADING_SPINNER_TICK_INTERVAL);

    // Creating a `Player` might be time consuming. It might take several seconds on first run.
    match Player::try_new() {
        Ok(mut player) => {
            player.set_volume(args.volume);
            PLAYER.lock().unwrap().replace(player);
        }
        Err(e) => {
            terminal::print_error(e);
            println!();
        }
    }

    loading_spinner.set_message("Connecting...");

    let (mut websocket_stream, _) = websocket_connect_task.await??;

    let message = get_next_websocket_message(&mut websocket_stream).await?;

    loading_spinner.finish_and_clear();

    let stations = get_stations_from_api_message(&message);

    let listen_url = match selected_station {
        Some(ref station) => stations
            .iter()
            .find(|s| s.id == station.id)
            .context(anyhow!("Station with ID \"{}\" not found", station.id))?
            .url
            .clone(),
        None => message.station.listen_url.clone(),
    };

    // Notify user if a new version is available
    if update_checking_task.is_finished() {
        if let Ok(Ok(Some(new_release))) = update_checking_task.await {
            println!(
                "{}",
                format!("New version available: {}", new_release.version).bright_yellow()
            );
            println!("{}", new_release.url.bright_yellow());
            println!();
        }
    }

    if let Some(station) = stations.iter().find(|station| station.url == listen_url) {
        println!("{}    {}", "Station:".bright_green(), station.name);
    }

    if let Some(player) = PLAYER.lock().unwrap().as_ref() {
        player.play(&listen_url);
    }

    let mut last_song_id = String::new();
    update_song_info_on_screen(message, &mut last_song_id);
    tokio::spawn(tick_progress_bar_progress());
    thread::spawn(handle_keyboard_input);

    loop {
        let message = get_next_websocket_message(&mut websocket_stream).await?;
        update_song_info_on_screen(message, &mut last_song_id);
    }
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
        println!("{logo}");
        println!();
    }
    println!("{description}");
    println!();
}

async fn get_next_websocket_message(
    websocket_stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
) -> Result<CodeRadioMessage> {
    if let Some(Ok(message)) = websocket_stream.next().await {
        if let Ok(message_text) = message.into_text() {
            if let Ok(code_radio_message) = serde_json::de::from_str(message_text.as_str()) {
                return Ok(code_radio_message);
            }
        }
    }

    // Cannot get message from WebSocket. Try to reconnect.
    let mut retry_count = 3;

    loop {
        match reconnect_websocket_and_get_next_message(websocket_stream).await {
            Ok(result) => return Ok(result),
            Err(error) => {
                retry_count -= 1;
                if retry_count == 0 {
                    return Err(error);
                }
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn reconnect_websocket_and_get_next_message(
    websocket_stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
) -> Result<CodeRadioMessage> {
    let _ = websocket_stream.close(None).await;
    let (new_websocket_stream, _) = tokio_tungstenite::connect_async(WEBSOCKET_API_URL).await?;
    *websocket_stream = new_websocket_stream;

    let message = websocket_stream
        .next()
        .await
        .context("Cannot get message from WebSocket")??;

    let code_radio_message: CodeRadioMessage =
        serde_json::de::from_str(message.into_text()?.as_str())?;

    Ok(code_radio_message)
}

/// Update progress bar's progress and listeners count suffix.
///
/// If song id changes, print the new song's info on screen.
///
/// Call this method when receiving a new message from Code Radio's WebSocket.
fn update_song_info_on_screen(message: CodeRadioMessage, last_song_id: &mut String) {
    let song = message.now_playing.song;

    let elapsed_seconds = message.now_playing.elapsed;
    let total_seconds = message.now_playing.duration; // Note: This may be 0

    let progress_bar_preffix =
        get_progress_bar_prefix(PLAYER.lock().unwrap().as_ref().map(Player::volume));
    let progress_bar_suffix = get_progress_bar_suffix(message.listeners.current);

    if song.id == *last_song_id {
        // Same song
        update_progress_bar(|p| {
            p.set_position(elapsed_seconds as u64);
            p.set_message(progress_bar_suffix);
        });
    } else {
        // New song
        update_progress_bar(ProgressBar::finish_and_clear);

        *last_song_id = song.id.clone();

        println!();
        println!("{}       {}", "Song:".bright_green(), song.title);
        println!("{}     {}", "Artist:".bright_green(), song.artist);
        println!("{}      {}", "Album:".bright_green(), song.album);

        let progress_bar_len = if total_seconds > 0 {
            total_seconds as u64
        } else {
            u64::MAX
        };

        let progress_bar_style =
            ProgressStyle::with_template("{prefix}  {wide_bar} {progress_info} - {msg}")
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

        PROGRESS_BAR.lock().unwrap().replace(progress_bar);
    }
}

fn get_progress_bar_prefix(volume: Option<u8>) -> String {
    let volume_char = volume.map_or_else(|| "*".to_owned(), |v| v.to_string());
    format!("Volume {volume_char}/9")
}

fn get_progress_bar_suffix(listener_count: i64) -> String {
    format!("Listeners: {listener_count}")
}

/// - If `elapsed_seconds` and `total_seconds` are both known:
///
///   `01:14 / 05:14`
///
/// - If `elapsed_seconds` is known but `total_seconds` is unknown:
///
///   `01:14`
fn get_progress_bar_progress_info(elapsed_seconds: u64, total_seconds: Option<u64>) -> String {
    let humanized_elapsed_duration =
        utils::humanize_seconds_to_minutes_and_seconds(elapsed_seconds);

    if let Some(total_seconds) = total_seconds {
        if total_seconds != u64::MAX {
            let humanized_total_duration =
                utils::humanize_seconds_to_minutes_and_seconds(total_seconds);
            return format!("{humanized_elapsed_duration} / {humanized_total_duration}");
        }
    }

    humanized_elapsed_duration
}

/// Increase elapsed seconds in progress bar by 1 every second.
async fn tick_progress_bar_progress() {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        update_progress_bar(|p| p.inc(1));
    }
}

fn update_progress_bar<T>(action: T)
where
    T: FnOnce(&ProgressBar),
{
    if let Some(progress_bar) = PROGRESS_BAR.lock().unwrap().as_ref() {
        action(progress_bar);
    }
}

/// When user press 0-9 on keyboard, adjust player volume.
fn handle_keyboard_input() -> ! {
    loop {
        if let Some(n) = terminal::read_char().ok().and_then(|c| c.to_digit(10)) {
            if let Some(player) = PLAYER.lock().unwrap().as_mut() {
                let volume = n as u8;
                if player.volume() == volume {
                    continue;
                }
                player.set_volume(volume);
                update_progress_bar(|p| p.set_prefix(get_progress_bar_prefix(Some(volume))));
            }
        }
    }
}

async fn select_station_interactively() -> Result<Remote> {
    let loading_spinner = ProgressBar::new_spinner()
        .with_style(ProgressStyle::with_template("{spinner} {msg}")?)
        .with_message("Connecting...");
    loading_spinner.enable_steady_tick(LOADING_SPINNER_TICK_INTERVAL);

    let stations = get_stations_from_rest_api().await?;

    loading_spinner.finish_and_clear();

    let station_names: Vec<&str> = stations.iter().map(|s| s.name.as_str()).collect();

    let selected_station_name = Select::new("Select a station:", station_names)
        .with_page_size(8)
        .prompt()?;
    let selected_station = stations
        .iter()
        .find(|s| s.name == selected_station_name)
        .unwrap()
        .clone();

    println!();

    Ok(selected_station)
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
