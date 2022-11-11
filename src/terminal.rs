use colored::Colorize;
use console::Term;
use once_cell::sync::Lazy;
use std::fmt::Display;

pub static STDOUT: Lazy<Term> = Lazy::new(|| Term::stdout());

pub fn enable_color_on_windows() {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).unwrap();
}

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
/// Unfortunately this may cause messy terminal output.
/// See `terminal::writeline!()` and `terminal::create_clean_up_helper()`'s doc.
pub fn read_char() -> std::io::Result<char> {
    STDOUT.read_char()
}

pub fn print_error(error: impl Display) {
    writeline!("{} {}", "Error:".bright_red(), error);
}

/// Whenever you want to print something to terminal, use this macro. DO NOT USE Rust's `println!()`.
///
/// # The Problem
///
/// On Unix-like OS, when `terminal::read_char()` is blocking in a background thread, your terminal will stay in "raw" mode.
/// If you write something to terminal from another thread, the terminal output will get messy:
///
/// - https://github.com/console-rs/console/issues/36
/// - https://github.com/console-rs/console/issues/136
///
/// See `terminal::read_char()`'s doc.
///
/// # The Workaround
///
/// This macro will move the cursor to the beginning of the line after writing a line, which fixes the bug.
macro_rules! writeline {
    () => {
        let _ = crate::terminal::STDOUT.write_line("\r");
    };
    ($($arg:tt)+) => {
        for line in format!($($arg)+).split("\n") {
            let line_r = format!("{}\r", line);
            let _ = crate::terminal::STDOUT.write_line(&line_r);
        }
    };
}

pub(crate) use writeline;

/// You should create an instance of `CleanUpHelper` by calling this method when the programs starts.
///
/// # The Problem
///
/// On Unix-like OS, if the program exits accidentally when `terminal::read_char()` is blocking,
/// your terminal will stay in "raw" mode and the terminal output will get messy.
///
/// See `terminal::read_char()`'s doc.
///
/// # The Workaround
///
/// This method will create an instance of `CleanUpHelper` struct, which implements `Drop` trait.
/// When it drops, it will send SIGINT (Ctrl+C) signal to the program itself on Unix-like OS, which fixes the bug.
/// Rust's Drop trait will guarantee the method to be called.
pub fn create_clean_up_helper() -> CleanUpHelper {
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
