mod args;
mod code_radio_api;
mod models;
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
use models::code_radio::{CodeRadioMessage, Remote};
use player::Player;
use rodio::Source;
use std::{fmt::Write, sync::Mutex, thread, time::Duration};

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

    // Fetching data in background while creating `Player` to improve startup speed
    // Note: Here we use the REST API to get the first API message,
    // because getting the first message from the Server-Sent Events stream may be slow
    let get_message_task = tokio::spawn(code_radio_api::get_message());
    let mut message_stream = code_radio_api::get_message_stream();

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
    let message = get_message_task.await??;
    loading_spinner.finish_and_clear();

    let stations = code_radio_api::get_stations_from_message(&message);

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

    while let Some(message) = message_stream.next().await {
        update_song_info_on_screen(message?, &mut last_song_id);
    }

    Err(anyhow!("Server-Sent Events connection was closed"))
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
        println!("{}", logo);
        println!();
    }
    println!("{}", description);
    println!();
}

/// Update progress bar's progress and listeners count suffix.
///
/// If song id changes, print the new song's info on screen.
///
/// Call this method when receiving a new message from Code Radio's Server-Sent Events stream.
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
        update_progress_bar(|p| p.finish_and_clear());

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

    let stations = code_radio_api::get_stations().await?;

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
