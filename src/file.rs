use nix::{
    errno::Errno,
    unistd::{self, AccessFlags},
};
use snafu::{Backtrace, IntoError, ResultExt, Snafu};
use std::{
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write as _},
    path::{Path, PathBuf},
};

#[derive(Debug, Snafu)]
pub(crate) enum Error {
    #[snafu(display("Could not open file {}: {}", filename.display(), source))]
    Open {
        filename: PathBuf,
        source: io::Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not read file {}: {}", filename.display(), source))]
    Read {
        filename: PathBuf,
        source: io::Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not write to file {}: {}", filename.display(), source))]
    Write {
        filename: PathBuf,
        source: io::Error,
        backtrace: Backtrace,
    },
    #[snafu(display("Could not get metadata of file {}: {}", filename.display(), source))]
    GetMetadata {
        filename: PathBuf,
        source: nix::Error,
        backtrace: Backtrace,
    },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn exists(filename: impl AsRef<Path>) -> bool {
    let filename = filename.as_ref();
    filename.exists()
}

pub(crate) fn writable(filename: impl AsRef<Path>) -> Result<bool> {
    let filename = filename.as_ref();
    match unistd::access(filename, AccessFlags::W_OK) {
        Ok(()) => Ok(true),
        Err(e) => {
            if let Some(Errno::EACCES) = e.as_errno() {
                return Ok(false);
            }
            Err(GetMetadata {
                filename: filename.to_path_buf(),
            }
            .into_error(e))
        }
    }
}

pub(crate) fn open(filename: impl AsRef<Path>) -> Result<Vec<String>> {
    let filename = filename.as_ref();
    let file = File::open(filename).with_context(|| Open {
        filename: filename.to_path_buf(),
    })?;

    let mut buf = vec![];

    let reader = BufReader::new(&file);
    for line in reader.lines() {
        let line = line.with_context(|| Read {
            filename: filename.to_path_buf(),
        })?;
        buf.push(line);
    }

    Ok(buf)
}

pub(crate) fn save(
    filename: impl AsRef<Path>,
    lines: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<usize> {
    let filename = filename.as_ref();

    let file = File::create(filename).with_context(|| Open {
        filename: filename.to_path_buf(),
    })?;
    let mut writer = BufWriter::new(file);

    let mut bytes = 0;
    for (idx, line) in lines.into_iter().enumerate() {
        if idx != 0 {
            writeln!(&mut writer).with_context(|| Write {
                filename: filename.to_path_buf(),
            })?;
            bytes += 1;
        }
        let line = line.as_ref();
        write!(&mut writer, "{}", line).with_context(|| Write {
            filename: filename.to_path_buf(),
        })?;
        bytes += line.len();
    }

    writer.flush().with_context(|| Write {
        filename: filename.to_path_buf(),
    })?;

    Ok(bytes)
}
