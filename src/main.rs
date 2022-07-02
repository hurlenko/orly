use clap::{Parser, ValueHint};
use fern::colors::{Color, ColoredLevelConfig};
use log::{error, info};
use orly::{
    client::{Authenticated, OreillyClient},
    epub::builder::EpubBuilder,
    error::Result,
    models::Book,
};
use sanitize_filename::sanitize;
use std::path::{Path, PathBuf};
use tokio::fs::File;

use anyhow::Context;

fn path_exists(v: &str) -> std::result::Result<(), String> {
    if Path::new(v).exists() {
        return Ok(());
    }
    Err(format!("The specifiied path does not exist: {}", v))
}

#[derive(Parser, Debug)]
#[clap(author, about, version)]
struct CliArgs {
    #[clap(
        short,
        long,
        value_names = &["EMAIL", "PASSWORD"],
        help = "Sign in credentials",
        required_unless_present = "cookie",
        conflicts_with = "cookie",
        number_of_values = 2
    )]
    creds: Option<Vec<String>>,
    #[clap(
        long,
        value_name = "COOKIE_STRING",
        help = "Cookie string",
        required_unless_present = "creds"
    )]
    cookie: Option<String>,
    #[clap(
        short,
        long,
        help = "Tweak css to avoid overflow. Useful for e-readers"
    )]
    kindle: bool,
    #[clap(
        short,
        long,
        help = "Sets the level of verbosity",
        parse(from_occurrences)
    )]
    verbose: u8,
    #[clap(
        short,
        long,
        help = "Sets the maximum number of concurrent http requests",
        default_value = "20"
    )]
    threads: usize,
    #[clap(help = "Book ID to download. Digits from the URL", required = true)]
    book_ids: Vec<String>,
    #[clap(
        short,
        long,
        help = "Directory to save the final epub to",
        name = "OUTPUT DIR",
        parse(from_os_str),
        value_hint = ValueHint::DirPath,
        default_value = ".",
        validator = path_exists,
    )]
    output: PathBuf,
}

fn generate_filename(book: &Book) -> String {
    let authors = book
        .authors
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<&str>>()
        .join("");

    let filename = if authors.is_empty() {
        format!("{} ({})", book.title, book.issued)
    } else {
        format!("{} ({}) - {}", book.title, book.issued, authors)
    };

    sanitize(filename)
}

async fn run(
    client: &OreillyClient<Authenticated>,
    book_id: &str,
    output: &Path,
    kindle: bool,
) -> Result<()> {
    info!("==== Getting book info =====");
    let book = client.fetch_book_details(book_id).await?;
    info!("Title: {:?}", book.title);
    info!(
        "Authors: {:?}",
        book.authors
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<&str>>()
            .join(", ")
    );

    let chapters = client.fetch_book_chapters(book_id).await?;

    info!("Downloaded {} chapters", chapters.len());

    let output = output.join(generate_filename(&book)).with_extension("epub");

    let file = File::create(&output)
        .await
        .context("Unable to create file")?;
    let toc = client.fetch_toc(book_id).await?;
    info!("Toc size: {}", toc.len());

    EpubBuilder::new(&book, kindle)?
        .chapters(&chapters)?
        .toc(&toc)?
        .generate(file, client)
        .await?;

    info!("Done! Saved as {:?}", output);

    Ok(())
}

fn set_up_logging(verbosity: u8) {
    let mut base_config = fern::Dispatch::new();

    base_config = match verbosity {
        0 => base_config.level(log::LevelFilter::Info),
        1 => base_config.level(log::LevelFilter::Debug),
        _ => base_config.level(log::LevelFilter::Trace),
    };

    // configure colors for the whole line
    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::White)
        .debug(Color::White)
        .trace(Color::BrightBlack);

    let colors_level = colors_line.info(Color::Green).debug(Color::BrightMagenta);

    base_config
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{color_line}[{date}][{level}{color_line}] {message}\x1B[0m",
                color_line = format_args!(
                    "\x1B[{}m",
                    colors_line.get_color(&record.level()).to_fg_str()
                ),
                date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                level = colors_level.color(record.level()),
                message = message,
            ));
        })
        .chain(std::io::stdout())
        .apply()
        .expect("failed to initialize logging.");
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli_args = CliArgs::parse();
    set_up_logging(cli_args.verbose);

    let client = OreillyClient::new(cli_args.threads);
    let client = if let Some(creds) = &cli_args.creds {
        client.cred_auth(&creds[0], &creds[1]).await?
    } else {
        client
            .cookie_auth(cli_args.cookie.as_ref().unwrap())
            .await?
    };

    for book_id in cli_args.book_ids.iter() {
        if let Err(err) = run(&client, book_id, &cli_args.output, cli_args.kindle).await {
            error!("{}", err)
        }
    }

    Ok(())
}
