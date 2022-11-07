# Code Radio CLI

[![Crate](https://img.shields.io/crates/v/code-radio-cli.svg)](https://crates.io/crates/code-radio-cli)
[![GitHub release (latest by date)](https://img.shields.io/github/v/release/JasonWei512/code-radio-cli)](https://github.com/JasonWei512/code-radio-cli/releases)

A command line music radio client for https://coderadio.freecodecamp.org, written in Rust.

![Screenshot](./.github/images/screenshot.jpg)

## Install

- Download prebuilt binary from [GitHub release page](https://github.com/JasonWei512/code-radio-cli/releases).

- Or install with [Cargo](https://rustup.rs/):
  
  ```
  cargo install code-radio-cli
  ```

  Then run `code-radio` in command line.

### ⚠ About building and running on Linux 🐧

This program uses [rustaudio/cpal](https://github.com/rustaudio/cpal) lib to play audio, which requires ALSA development files on Linux.

In order to build and run this program on Linux, you need to install：

- `libasound2-dev` on Debian / Ubuntu
- `alsa-lib-devel` on Fedora

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