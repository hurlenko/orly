use clap::{Clap, ValueHint};
use orly::{client::OreillyClient, epub::builder::EpubBuilder, error::Result, models::Book};
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

#[derive(Clap, Debug)]
#[clap(author, about, version)]
struct Opt {
    #[clap(
        short,
        long,
        value_name = "EMAIL PASSWORD",
        about = "Sign in credentials",
        required = true,
        number_of_values = 2
    )]
    creds: Vec<String>,
    #[clap(
        short,
        long,
        about = "Tweak css to avoid overflow. Useful for e-readers"
    )]
    kindle: bool,
    #[clap(
        short,
        long,
        about = "Sets the level of verbosity",
        parse(from_occurrences)
    )]
    verbose: u8,
    #[clap(
        short,
        long,
        about = "Sets the maximum number of concurrent http requests",
        default_value = "20"
    )]
    threads: usize,
    #[clap(about = "Book ID to download. Digits from the URL", required = true)]
    book_id: String,
    #[clap(
        short,
        long,
        about = "Directory to save the final epub to",
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

async fn run() -> Result<()> {
    let cli_args = Opt::parse();

    println!("{:#?}", cli_args);

    let email = &cli_args.creds[0];
    let password = &cli_args.creds[1];
    let book_id = &cli_args.book_id;

    let client = OreillyClient::new(cli_args.threads)
        .cred_auth(email, password)
        .await?;

    let book = client.fetch_book_details(book_id).await?;
    println!("{:#?}", book);

    let chapters = client.fetch_book_chapters(book_id).await?;

    println!("Downloaded {} chapters", chapters.len());

    let output = cli_args
        .output
        .join(generate_filename(&book))
        .with_extension("epub");

    let file = File::create(&output)
        .await
        .context("Unable to create file")?;
    let toc = client.fetch_toc(book_id).await?;
    println!("Downloaded toc: {:?}", toc.len());

    EpubBuilder::new(&book, cli_args.kindle)?
        .chapters(&chapters)?
        .toc(&toc)?
        .generate(file, client)
        .await?;

    println!("Done! Saved as {:?}", output);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(err) = run().await {
        eprintln!("{}", err)
    }

    Ok(())
}
