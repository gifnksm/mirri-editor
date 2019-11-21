use crate::{decode::Decoder, editor::Editor, terminal::RawTerminal};
use snafu::{ErrorCompat, ResultExt, Snafu};
use std::{path::PathBuf, process};
use structopt::StructOpt;

mod decode;
mod editor;
mod file;
mod find;
mod geom;
mod input;
mod output;
mod render;
mod row;
mod signal;
mod status_message;
mod syntax;
mod terminal;
mod text_buffer;
mod util;
mod welcome;

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
    let mut term = RawTerminal::new(&mut decoder).context(Terminal)?;

    let mut render_size = term.screen_size;
    render_size.rows -= 2;
    let mut editor = Editor::new(render_size);

    editor.set_status_message("HELP: Ctrl-S = save | Ctrl-Q = quit | Ctrl-G = find");

    if let Some(file) = &opt.file {
        editor.open(file);
    }

    loop {
        output::refresh_screen(&mut term, &mut decoder, &mut editor).context(Output)?;
        output::flush(&mut term).context(Output)?;

        if input::process_keypress(&mut term, &mut decoder, &mut editor).context(Input)? {
            break;
        }
    }

    output::clear_screen(&mut term).context(Output)?;
    output::flush(&mut term).context(Output)?;

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
