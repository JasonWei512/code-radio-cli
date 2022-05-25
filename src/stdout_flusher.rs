use std::io::{self, Write};

pub fn flush_stdout() {
    io::stdout().flush().unwrap();
}