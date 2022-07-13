<!-- omit in toc -->
# orly

Download O'Reilly books as EPUB.

![GitHub release](https://img.shields.io/github/v/release/hurlenko/orly)
![Downloads](https://img.shields.io/github/downloads/hurlenko/orly/latest/total)
![Crates.io](https://img.shields.io/crates/d/orly)
![License](https://img.shields.io/badge/license-MIT-blue.svg)

<!-- omit in toc -->
## Table of Contents

- [Installation](#installation)
  - [Github releases (recommended)](#github-releases-recommended)
  - [Cargo](#cargo)
- [Usage](#usage)
- [Command line interface](#command-line-interface)

## Installation

### Github releases (recommended)

**[Archives of precompiled binaries for orly are available for Windows,
macOS and Linux.](https://github.com/hurlenko/orly/releases)** Linux and
Windows binaries are static executables.

### Cargo

If you're a **Rust programmer**, orly can be installed with `cargo`.

    > Note that the minimum supported version of Rust for `orly` is **1.54.0**.

You need to install the development headers of `libxml2` first. The process depends on the OS being used:

- Windows

    First install [vcpkg](https://github.com/microsoft/vcpkg). After that install `libxml2`:

    ```bash
    vcpkg install libxml2:x64-windows-static
    ```

    Export compiler options to force static linking:
    ```bash
    $env:RUSTFLAGS="-Ctarget-feature=+crt-static"
    ```

- Linux

    On linux systems you'd `pkg-config`. For Debian-based distributions:

    ```bash
    apt-get install libxml2-dev pkg-config
    ```

- macOS
  
    Use `brew` to install `libxml2` and `pkg-config`:

    ```bash
    brew install libxml2 pkg-config
    ```

Finally install `orly`:

```bash
cargo install orly
```

After installation, the `orly` command will be available. Check the [command line](#command-line-interface) section for supported commands.

## Usage

- You will need an O'Reily account with a non-expired subscription.

- Find the book you want to download and copy its id (the digits at the end of the url).

- Use your credentials or a cookie string to download the book:

    ```bash
    orly 1234567890 --creds "email@example.com" "password"
    # or
    orly 1234567890 --cookie 'BrowserCookie=....'
    ```

## Command line interface

Currently `orly` supports these commands

```bash
USAGE:
    orly [OPTIONS] <BOOK_IDS>...

ARGS:
    <BOOK_IDS>...    Book ID to download. Digits from the URL

OPTIONS:
    -c, --creds <EMAIL> <PASSWORD>    Sign in credentials
        --cookie <COOKIE_STRING>      Cookie string
    -h, --help                        Print help information
    -k, --kindle                      Apply CSS tweaks for kindle devices
    -o, --output <OUTPUT DIR>         Directory to save the final epub to [default: .]
    -t, --threads <THREADS>           Maximum number of concurrent http requests [default: 20]
    -v, --verbose                     Level of verbosity
    -V, --version                     Print version information
```
