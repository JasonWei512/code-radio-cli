use clap::{
    builder::{styling::AnsiColor, Styles},
    Parser,
};

const ABOUT: &str = "A command line music radio client for https://coderadio.freecodecamp.org
GitHub: https://github.com/JasonWei512/code-radio-cli";

const STYLE: Styles = Styles::styled()
    .header(AnsiColor::Yellow.on_default())
    .usage(AnsiColor::Yellow.on_default())
    .literal(AnsiColor::Green.on_default())
    .placeholder(AnsiColor::Green.on_default());

#[derive(Parser, Debug)]
#[clap(author, version, about = ABOUT, styles = STYLE)]
pub struct Args {
    /// Manually select a station
    #[clap(short, long)]
    pub select_station: bool,

    /// Volume, between 0 and 9
    #[clap(short, long, default_value_t = 9)]
    pub volume: u8,

    /// Do not display logo
    #[clap(short, long)]
    pub no_logo: bool,
}
