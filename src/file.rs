use crate::editor::Editor;
use snafu::{Backtrace, ResultExt, Snafu};
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Write},
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
    #[snafu(display("Could not write: {}", source))]
    FileWrite {
        filename: PathBuf,
        source: io::Error,
        backtrace: Backtrace,
    },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn open(editor: &mut Editor, filename: impl Into<PathBuf>) -> Result<()> {
    let filename = filename.into();
    let file = File::open(&filename).with_context(|| FileOpen {
        filename: filename.clone(),
    })?;

    let reader = BufReader::new(&file);
    for line in reader.lines() {
        let line = line.with_context(|| FileRead {
            filename: filename.clone(),
        })?;
        editor.append_row(line);
    }

    editor.filename = Some(filename);
    editor.dirty = false;
    Ok(())
}

pub(crate) fn save(
    filename: impl AsRef<Path>,
    lines: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<usize> {
    let filename = filename.as_ref();
    let mut file = File::create(filename).with_context(|| FileOpen {
        filename: filename.to_path_buf(),
    })?;

    let mut bytes = 0;
    for line in lines {
        let line = line.as_ref();
        writeln!(&mut file, "{}", line).with_context(|| FileWrite {
            filename: filename.to_path_buf(),
        })?;
        bytes += line.len() + 1; // sizeof line + '\n'
    }

    file.flush().with_context(|| FileWrite {
        filename: filename.to_path_buf(),
    })?;

    Ok(bytes)
}
