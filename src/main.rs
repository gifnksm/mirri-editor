use crate::editor::Editor;
use snafu::{ErrorCompat, ResultExt, Snafu};
use std::process;

mod editor;
mod input;
mod output;
mod terminal;

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

fn run() -> Result<()> {
    let mut editor = Editor::new().context(EditorError)?;
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
