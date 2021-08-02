use std::fmt;
use std::io;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::path::Path;

use zip::result::ZipResult;
use zip::write::{FileOptions, ZipWriter};
use zip::CompressionMethod;

pub struct ZipLibrary {
    writer: ZipWriter<Cursor<Vec<u8>>>,
}

impl fmt::Debug for ZipLibrary {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ZipLibrary")
    }
}

impl ZipLibrary {
    /// Creates a new wrapper for zip library
    ///
    /// Also add mimetype at the beginning of the EPUB file.
    pub fn new() -> ZipResult<ZipLibrary> {
        let mut writer = ZipWriter::new(Cursor::new(vec![]));
        writer.set_comment(""); // Fix issues with some readers

        writer
            .start_file(
                "mimetype",
                FileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .chain_err(|| format!("could not create mimetype in epub"))?;
        writer
            .write(b"application/epub+zip")
            .chain_err(|| format!("could not write mimetype in epub"))?;

        Ok(ZipLibrary { writer: writer })
    }

    fn write_file<P: AsRef<Path>, R: Read>(&mut self, path: P, mut content: R) -> ZipResult<()> {
        let mut file = format!("{}", path.as_ref().display());
        if cfg!(target_os = "windows") {
            // Path names should not use backspaces in zip files
            file = file.replace('\\', "/");
        }
        let options = FileOptions::default();
        self.writer
            .start_file(file.clone(), options)
            .chain_err(|| format!("could not create file '{}' in epub", file))?;
        io::copy(&mut content, &mut self.writer)
            .chain_err(|| format!("could not write file '{}' in epub", file))?;
        Ok(())
    }

    fn generate<W: Write>(&mut self, mut to: W) -> ZipResult<()> {
        let cursor = self
            .writer
            .finish()
            .chain_err(|| "error writing zip file")?;
        let bytes = cursor.into_inner();
        to.write_all(bytes.as_ref())
            .chain_err(|| "error writing zip file")?;
        Ok(())
    }
}
