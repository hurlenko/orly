use std::fmt;
use std::io;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

use crate::error::Result;
use anyhow::Context;
use zip::write::{FileOptions, ZipWriter};
use zip::CompressionMethod;

pub struct ZipArchive {
    writer: ZipWriter<Cursor<Vec<u8>>>,
}

impl fmt::Debug for ZipArchive {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ZipArchive")
    }
}

impl ZipArchive {
    pub fn new() -> Result<Self> {
        let mut writer = ZipWriter::new(Cursor::new(vec![]));
        writer.set_comment(""); // Fix issues with some readers

        writer
            .start_file(
                "mimetype",
                FileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .context("could not create mimetype in epub")?;
        writer
            .write(b"application/epub+zip")
            .context("could not write mimetype in epub")?;

        Ok(ZipArchive { writer })
    }

    pub fn write_file<P: AsRef<Path>, R: Read>(&mut self, path: P, mut content: R) -> Result<()> {
        let mut file = format!("{}", path.as_ref().display());
        if cfg!(target_os = "windows") {
            // Path names should not use backspaces in zip files
            file = file.replace('\\', "/");
        }
        let options = FileOptions::default();
        self.writer
            .start_file(file.clone(), options)
            .with_context(|| format!("could not create file '{}' in epub", file))?;
        io::copy(&mut content, &mut self.writer)
            .with_context(|| format!("could not write file '{}' in epub", file))?;
        Ok(())
    }

    pub async fn generate<W: AsyncWrite + std::marker::Unpin>(&mut self, mut to: W) -> Result<()> {
        let cursor = self
            .writer
            .finish()
            .with_context(|| "error writing zip file")?;
        let bytes = cursor.into_inner();
        to.write_all(bytes.as_ref())
            .await
            .with_context(|| "error writing zip file")?;
        Ok(())
    }
}
