use crate::editor::Editor;
use snafu::{ErrorCompat, ResultExt, Snafu};
use std::{path::PathBuf, process};
use structopt::StructOpt;

mod editor;
mod file;
mod find;
mod input;
mod output;
mod row;
mod signal;
mod syntax;
mod terminal;
mod util;

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("{}", source))]
    EditorError { source: editor::Error },
    #[snafu(display("{}", source))]
    InputError { source: input::Error },
    #[snafu(display("{}", source))]
    OutputError { source: output::Error },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, StructOpt)]
struct Opt {
    /// File to process
    #[structopt(name = "FILE", parse(from_os_str))]
    file: Option<PathBuf>,
}

fn run() -> Result<()> {
    let opt = Opt::from_args();
    let mut editor = Editor::new().context(EditorError)?;
    editor.set_status_msg("HELP: Ctrl-S = save | Ctrl-Q = quit | Ctrl-F = find");

    if let Some(file) = &opt.file {
        editor.open(file);
    }

    loop {
        output::refresh_screen(&mut editor).context(OutputError)?;
        output::flush(&mut editor).context(OutputError)?;

        if input::process_keypress(&mut editor).context(InputError)? {
            break;
        }
    }

    output::clear_screen(&mut editor).context(OutputError)?;
    output::flush(&mut editor).context(OutputError)?;

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("An error occurred: {}", e);
        if let Some(backtrace) = ErrorCompat::backtrace(&e) {
            eprintln!("{}", backtrace);
        }
        process::exit(1);
    }
}
