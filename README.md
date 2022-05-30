# Code Radio CLI

[![Crate](https://img.shields.io/crates/v/code-radio-cli.svg)](https://crates.io/crates/code-radio-cli)

A command line music radio client for https://coderadio.freecodecamp.org

## Install

Install [Rust](https://rustup.rs/), then run:

```
cargo install code-radio-cli
```

## Usage

```
code-radio [OPTIONS]

OPTIONS:
    -h, --help                 Print help information
    -l, --list-stations        List all stations
    -n, --no-logo              Do not display logo
    -s, --station <STATION>    The ID of the station to play from
    -v, --volume <VOLUME>      Volume, between 0 and 9 [default: 9]
    -V, --version              Print version information
```