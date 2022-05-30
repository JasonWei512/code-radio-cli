use console::Term;
use once_cell::sync::Lazy;

pub static TERM: Lazy<Term> = Lazy::new(|| Term::stdout());

pub fn read_char() -> std::io::Result<char> {
    TERM.read_char()
}

// This is a workaround for https://github.com/console-rs/console/issues/36
macro_rules! writeline {
    () => {
        let _ = terminal::TERM.write_line("\r");
    };
    ($($arg:tt)+) => {
        for line in format!($($arg)+).split("\n") {
            let line_r = format!("{}\r", line);
            let _ = terminal::TERM.write_line(&line_r);
        }
    };
}

pub(crate) use writeline;
