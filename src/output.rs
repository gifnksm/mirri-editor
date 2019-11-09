use crate::editor::Editor;
use snafu::{Backtrace, ResultExt, Snafu};
use std::io::{self, Write};

#[derive(Debug, Snafu)]
pub(crate) enum Error {
    #[snafu(display("Could not output to terminal: {}", source))]
    TerminalOutput {
        source: io::Error,
        backtrace: Backtrace,
    },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn clear_screen(editor: &mut Editor) -> Result<()> {
    // ED - Erase In Display
    //   <esc> [ <param> J
    // Params:
    //   0 : clear the screen from the cursor up to the end of the screen
    //   1 : clear the screen up to where the cursor is
    //   2 : clear the entire screen
    write!(&mut editor.term, "\x1b[2J").context(TerminalOutput)?;

    // CUP - Cursor Position
    //   <esc> [ <row> ; <col> H
    // if params are omitted, the cursor will be positioned at the first row and first column (col=1, row=1)
    write!(&mut editor.term, "\x1b[H").context(TerminalOutput)?;

    Ok(())
}

fn scroll(editor: &mut Editor) {
    editor.rx = if let Some(row) = editor.rows.get(editor.cy) {
        row.cx_to_rx(editor.cx)
    } else {
        0
    };

    if editor.cy < editor.row_off {
        editor.row_off = editor.cy;
    }
    if editor.cy >= editor.row_off + editor.screen_rows {
        editor.row_off = editor.cy - editor.screen_rows + 1;
    }
    if editor.rx < editor.col_off {
        editor.col_off = editor.rx;
    }
    if editor.rx >= editor.col_off + editor.screen_cols {
        editor.col_off = editor.rx - editor.screen_cols + 1;
    }
}

fn draw_raws(editor: &mut Editor) -> Result<()> {
    scroll(editor);

    for y in 0..editor.screen_rows {
        let file_row = y + editor.row_off;
        if file_row >= editor.rows.len() {
            if y == editor.screen_rows / 3 {
                let welcome = format!(
                    "{} -- version {}",
                    env!("CARGO_PKG_DESCRIPTION"),
                    env!("CARGO_PKG_VERSION")
                );
                let mut width = editor.screen_cols;
                if welcome.len() < editor.screen_cols {
                    write!(&mut editor.term, "~").context(TerminalOutput)?;
                    width = editor.screen_cols - 1
                }
                write!(&mut editor.term, "{:^w$.p$}", welcome, w = width, p = width)
                    .context(TerminalOutput)?;
            } else {
                write!(&mut editor.term, "~").context(TerminalOutput)?;
            }
        } else {
            let row = &editor.rows[file_row];
            if row.render.len() > editor.col_off {
                write!(
                    &mut editor.term,
                    "{:.p$}",
                    &row.render[editor.col_off..],
                    p = editor.screen_cols
                )
                .context(TerminalOutput)?;
            }
        }

        // EL - Erase In Line
        //  <esc> [ <param> K
        // Params:
        //  0 : erase from active position to the end of the line, inclusive (default)
        //  1 : erase from the start of the screen to the active position, inclusive
        //  2 : erase all of the line, inclusive
        write!(&mut editor.term, "\x1b[K").context(TerminalOutput)?;

        if y + 1 < editor.screen_rows {
            writeln!(&mut editor.term, "\r").context(TerminalOutput)?;
        }
    }

    Ok(())
}

pub(crate) fn refresh_screen(editor: &mut Editor) -> Result<()> {
    write!(&mut editor.term, "\x1b[?25l").context(TerminalOutput)?; // hide cursor
    write!(&mut editor.term, "\x1b[H").context(TerminalOutput)?; // move cursor to top-left corner

    draw_raws(editor)?;

    write!(
        &mut editor.term,
        "\x1b[{};{}H",
        (editor.cy - editor.row_off) + 1,
        (editor.rx - editor.col_off) + 1
    )
    .context(TerminalOutput)?; // move cursor
    write!(&mut editor.term, "\x1b[?25h").context(TerminalOutput)?; // show cursor

    Ok(())
}

pub(crate) fn flush(editor: &mut Editor) -> Result<()> {
    editor.term.flush().context(TerminalOutput)?;

    Ok(())
}
