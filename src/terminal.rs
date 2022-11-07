use colored::Colorize;
use console::Term;
use once_cell::sync::Lazy;
use std::fmt::Display;

pub static TERM: Lazy<Term> = Lazy::new(|| Term::stdout());

pub fn enable_color_on_windows() {
    #[cfg(target_os = "windows")]
    colored::control::set_virtual_terminal(true).unwrap();
}

pub fn read_char() -> std::io::Result<char> {
    TERM.read_char()
}

pub fn print_error(error: impl Display) {
    writeline!("{} {}", "Error:".bright_red(), error);
}

// This is a workaround for https://github.com/console-rs/console/issues/36
macro_rules! writeline {
    () => {
        let _ = crate::terminal::TERM.write_line("\r");
    };
    ($($arg:tt)+) => {
        for line in format!($($arg)+).split("\n") {
            let line_r = format!("{}\r", line);
            let _ = crate::terminal::TERM.write_line(&line_r);
        }
    };
}

pub(crate) use writeline;
