use colored::Colorize;
use console::Term;
use once_cell::sync::Lazy;
use std::fmt::Display;

static STDOUT: Lazy<Term> = Lazy::new(Term::stdout);

pub fn enable_color_on_windows() {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).unwrap();
}

pub fn read_char() -> std::io::Result<char> {
    STDOUT.read_char()
}

pub fn print_error(error: impl Display) {
    println!("{} {}", "Error:".bright_red(), error);
}

/// You should create an instance of `CleanUpHelper` by calling this method when the programs starts.
///
/// # The Problem
///
/// This program handles keyboard input (adjust volume) by spawning a thread
/// and calling [console](https://github.com/console-rs/console) crate's `console::Term::stdout().read_char()` in a loop.
///
/// This is how `console::Term::stdout().read_char()` works on Unix-like OS:
/// 1. Call the method
/// 2. Your terminal exits "canonical" mode and enters "raw" mode
/// 3. The method blocks until you press a key
/// 4. Terminal exits "raw" mode and returns to "canonical" mode
/// 5. The method returns the key you pressed
///
/// Unfortunately, on Unix-like OS, if the program exits accidentally when `terminal::read_char()` is blocking,
/// your terminal will stay in "raw" mode and the terminal output will get messy:
///
/// - https://github.com/console-rs/console/issues/36
/// - https://github.com/console-rs/console/issues/136
///
/// # The Workaround
///
/// This method will create an instance of `CleanUpHelper` struct, which implements `Drop` trait.
/// When it drops, it will send SIGINT (Ctrl+C) signal to the program itself on Unix-like OS, which fixes the bug.
/// Rust's Drop trait will guarantee the method to be called.
pub const fn create_clean_up_helper() -> CleanUpHelper {
    CleanUpHelper {}
}

pub struct CleanUpHelper {}

impl Drop for CleanUpHelper {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::raise(libc::SIGINT);
        }
    }
}
