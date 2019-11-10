use crate::{editor::Editor, syntax::Highlight};
use snafu::{Backtrace, ResultExt, Snafu};
use std::{
    cmp,
    ffi::OsStr,
    io::{self, Write},
};

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

fn draw_rows(editor: &mut Editor) -> Result<()> {
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
            let row = &mut editor.rows[file_row];
            row.update_render();
            row.update_syntax(editor.syntax);
            let render = row.render();
            let hl = row.highlight();

            let mut current_color = None;
            if render.len() > editor.col_off {
                for (idx, ch) in render
                    [editor.col_off..cmp::min(editor.col_off + editor.screen_cols, render.len())]
                    .char_indices()
                {
                    let hl = hl[idx];
                    if hl == Highlight::Normal {
                        if current_color.is_some() {
                            current_color = None;
                            write!(&mut editor.term, "\x1b[39m").context(TerminalOutput)?;
                        }
                        write!(&mut editor.term, "{}", ch).context(TerminalOutput)?;
                    } else {
                        let color = hl.to_color();
                        if current_color != Some(color) {
                            current_color = Some(color);
                            write!(&mut editor.term, "\x1b[{}m", color).context(TerminalOutput)?;
                        }
                        write!(&mut editor.term, "{}", ch).context(TerminalOutput)?;
                    }
                }
                write!(&mut editor.term, "\x1b[39m").context(TerminalOutput)?;
            }
        }

        // EL - Erase In Line
        //  <esc> [ <param> K
        // Params:
        //  0 : erase from active position to the end of the line, inclusive (default)
        //  1 : erase from the start of the screen to the active position, inclusive
        //  2 : erase all of the line, inclusive
        write!(&mut editor.term, "\x1b[K").context(TerminalOutput)?;
        writeln!(&mut editor.term, "\r").context(TerminalOutput)?;
    }

    Ok(())
}

fn draw_status_bar(editor: &mut Editor) -> Result<()> {
    let default_path = OsStr::new("[No Name]");
    let path = editor
        .filename
        .as_ref()
        .and_then(|p| p.file_name())
        .unwrap_or(default_path);
    let dirty_indicator = if editor.dirty { "(modified)" } else { "" };

    let l_status = format!(
        "{:.20} - {} lines {}",
        path.to_string_lossy(),
        editor.rows.len(),
        dirty_indicator
    );
    let r_status = format!(
        "{} | {}/{}",
        editor.syntax.filetype,
        editor.cy + 1,
        editor.rows.len()
    );

    let l_width = cmp::min(l_status.len(), editor.screen_cols);
    let r_width = cmp::min(r_status.len(), editor.screen_cols - l_width);
    let sep_width = editor.screen_cols - l_width - r_width;

    write!(
        &mut editor.term,
        "\x1b[7m{:.wl$}{:ws$}{:.wr$}\x1b[m",
        l_status,
        "",
        r_status,
        wl = l_width,
        ws = sep_width,
        wr = r_width,
    )
    .context(TerminalOutput)?;
    writeln!(&mut editor.term, "\r").context(TerminalOutput)?;
    Ok(())
}

fn draw_message_bar(editor: &mut Editor) -> Result<()> {
    write!(&mut editor.term, "\x1b[K").context(TerminalOutput)?;
    if let Some((time, msg)) = &mut editor.status_msg {
        if time.elapsed().as_secs() < 5 {
            write!(&mut editor.term, "{:.w$}", msg, w = editor.screen_cols)
                .context(TerminalOutput)?;
        } else {
            editor.status_msg = None;
        }
    }
    Ok(())
}

pub(crate) fn refresh_screen(editor: &mut Editor) -> Result<()> {
    write!(&mut editor.term, "\x1b[?25l").context(TerminalOutput)?; // hide cursor
    write!(&mut editor.term, "\x1b[H").context(TerminalOutput)?; // move cursor to top-left corner

    draw_rows(editor)?;
    draw_status_bar(editor)?;
    draw_message_bar(editor)?;

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
