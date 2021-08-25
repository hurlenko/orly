# orly

Download O'Reilly books as EPUB.

![GitHub release](https://img.shields.io/github/v/release/hurlenko/orly)
![Downloads](https://img.shields.io/github/downloads/hurlenko/orly/latest/total)
![Crates.io](https://img.shields.io/crates/d/orly)
![License](https://img.shields.io/badge/license-MIT-blue.svg)

## Table of Contents

- [Installation](#installation)
- [Usage](#usage)
- [Command line interface](#command-line-interface)

## Installation

- **[Archives of precompiled binaries for orly are available for Windows,
macOS and Linux.](https://github.com/hurlenko/orly/releases)** Linux and
Windows binaries are static executables.

- If you're a **Rust programmer**, orly can be installed with `cargo`.

    > Note that the minimum supported version of Rust for `orly` is **1.54.0**.

    ```bash
    cargo install orly
    ```

After installation, the `orly` command will be available. Check the [command line](#command-line-interface) section for supported commands.

## Usage

- You will need an O'Reily account with a non-expired subscription.

- Find the book you want to download and copy its id (the digits at the end of the url).

- Use your credentials to download the book:

    ```bash
    orly --creds "email@example.com" "password" 1234567890
    ```

## Command line interface

Currently `orly` supports these commands

```bash
USAGE:
    orly.exe [FLAGS] [OPTIONS] --creds <EMAIL PASSWORD>... <BOOK_ID>

ARGS:
    <BOOK_ID>    Book ID to download. Digits from the URL

FLAGS:
    -h, --help       Print help information
    -k, --kindle     Tweak css to avoid overflow. Useful for e-readers
    -v, --verbose    Sets the level of verbosity
    -V, --version    Print version information

OPTIONS:
    -c, --creds <EMAIL> <PASSWORD>     Sign in credentials
    -o, --output <OUTPUT DIR>          Directory to save the final epub to [default: .]
    -t, --threads <THREADS>            Sets the maximum number of concurrent http requests [default: 20]
```
