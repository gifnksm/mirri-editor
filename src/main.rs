use crate::{decode::Decoder, editor::Editor, terminal::RawTerminal};
use snafu::{ErrorCompat, ResultExt, Snafu};
use std::{path::PathBuf, process};
use structopt::StructOpt;

mod decode;
mod editor;
mod file;
mod find;
mod input;
mod output;
mod row;
mod signal;
mod syntax;
mod terminal;
mod text_buffer;
mod util;

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("{}", source))]
    Terminal { source: terminal::Error },
    #[snafu(display("{}", source))]
    Input { source: input::Error },
    #[snafu(display("{}", source))]
    Output { source: output::Error },
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

    let mut decoder = Decoder::new();
    let term = RawTerminal::new(&mut decoder).context(Terminal)?;

    let render_rows = term.screen_rows - 2;
    let render_cols = term.screen_cols;
    let mut editor = Editor::new(decoder, term, render_rows, render_cols);

    editor.set_status_msg("HELP: Ctrl-S = save | Ctrl-Q = quit | Ctrl-G = find");

    if let Some(file) = &opt.file {
        editor.open(file);
    }

    loop {
        output::refresh_screen(&mut editor).context(Output)?;
        output::flush(&mut editor).context(Output)?;

        if input::process_keypress(&mut editor).context(Input)? {
            break;
        }
    }

    output::clear_screen(&mut editor).context(Output)?;
    output::flush(&mut editor).context(Output)?;

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
