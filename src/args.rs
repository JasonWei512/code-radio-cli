use clap::Parser;

const ABOUT: &str = "A command line music radio client for https://coderadio.freecodecamp.org
GitHub: https://github.com/JasonWei512/code-radio-cli";

#[derive(Parser, Debug)]
#[clap(author, version, about = ABOUT)]
pub struct Args {
    /// The ID of the station to play from
    #[clap(short, long)]
    pub station: Option<i64>,

    /// List all stations
    #[clap(short, long)]
    pub list_stations: bool,

    /// Volume, between 0 and 9
    #[clap(short, long, default_value_t = 9)]
    pub volume: u8,

    /// Do not display logo
    #[clap(short, long)]
    pub no_logo: bool,
}
