use crate::editor::Editor;
use snafu::{Backtrace, ResultExt, Snafu};
use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
};

#[derive(Debug, Snafu)]
pub(crate) enum Error {
    #[snafu(display("Could not open file {}: {}", filename.display(), source))]
    FileOpen {
        filename: PathBuf,
        source: io::Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not read file {}: {}", filename.display(), source))]
    FileRead {
        filename: PathBuf,
        source: io::Error,
        backtrace: Backtrace,
    },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn open(editor: &mut Editor, filename: impl AsRef<Path>) -> Result<()> {
    let filename = filename.as_ref();
    let file = File::open(filename).with_context(|| FileOpen {
        filename: filename.to_path_buf(),
    })?;

    let reader = BufReader::new(&file);
    for line in reader.lines() {
        let line = line.with_context(|| FileRead {
            filename: filename.to_path_buf(),
        })?;
        editor.append_row(line);
    }

    Ok(())
}
