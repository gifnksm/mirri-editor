use crate::{
    decode::Decoder,
    editor::Editor,
    syntax::Highlight,
    terminal::{self, RawTerminal},
    text_buffer::{Status, TextBuffer},
};
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
    #[snafu(display("{}", source))]
    Terminal {
        source: terminal::Error,
        backtrace: Backtrace,
    },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn clear_screen(term: &mut RawTerminal) -> Result<()> {
    // ED - Erase In Display
    //   <esc> [ <param> J
    // Params:
    //   0 : clear the screen from the cursor up to the end of the screen
    //   1 : clear the screen up to where the cursor is
    //   2 : clear the entire screen
    write!(term, "\x1b[2J").context(TerminalOutput)?;

    // CUP - Cursor Position
    //   <esc> [ <row> ; <col> H
    // if params are omitted, the cursor will be positioned at the first row and first column (col=1, row=1)
    write!(term, "\x1b[H").context(TerminalOutput)?;

    Ok(())
}

fn draw_main(term: &mut RawTerminal, editor: &Editor) -> Result<()> {
    if let Some(buffer) = &editor.buffer {
        draw_rows(term, buffer)?;
    } else {
        draw_welcome(term, editor)?;
    }
    Ok(())
}

fn draw_welcome(term: &mut RawTerminal, editor: &Editor) -> Result<()> {
    let render_size = editor.render_size();
    for y in 0..render_size.rows {
        if y == render_size.rows / 3 {
            let welcome = format!(
                "{} -- version {}",
                env!("CARGO_PKG_DESCRIPTION"),
                env!("CARGO_PKG_VERSION")
            );
            let mut width = term.screen_size.cols;
            if welcome.len() < term.screen_size.cols {
                write!(term, "~").context(TerminalOutput)?;
                width = term.screen_size.cols - 1
            }
            write!(term, "{:^w$.p$}", welcome, w = width, p = width).context(TerminalOutput)?;
        } else {
            write!(term, "~").context(TerminalOutput)?;
        }

        // EL - Erase In Line
        //  <esc> [ <param> K
        // Params:
        //  0 : erase from active position to the end of the line, inclusive (default)
        //  1 : erase from the start of the screen to the active position, inclusive
        //  2 : erase all of the line, inclusive
        write!(term, "\x1b[K").context(TerminalOutput)?;
        writeln!(term, "\r").context(TerminalOutput)?;
    }
    Ok(())
}

fn draw_rows(term: &mut RawTerminal, buffer: &TextBuffer) -> Result<()> {
    for row_render in buffer.render_with_highlight() {
        let mut current_color = None;
        for (hl, item) in row_render {
            if hl == Highlight::Normal {
                if current_color.is_some() {
                    current_color = None;
                    write!(term, "\x1b[39;49m").context(TerminalOutput)?;
                }
            } else {
                let color = hl.to_color();
                if current_color != Some(color) {
                    current_color = Some(color);
                    write!(term, "\x1b[{};{}m", color.0, color.1).context(TerminalOutput)?;
                }
            }
            write!(term, "{}", item).context(TerminalOutput)?;
        }
        if current_color.is_some() {
            write!(term, "\x1b[39;49m").context(TerminalOutput)?;
        }
        // EL - Erase In Line
        //  <esc> [ <param> K
        // Params:
        //  0 : erase from active position to the end of the line, inclusive (default)
        //  1 : erase from the start of the screen to the active position, inclusive
        //  2 : erase all of the line, inclusive
        write!(term, "\x1b[K").context(TerminalOutput)?;
        writeln!(term, "\r").context(TerminalOutput)?;
    }

    Ok(())
}

fn draw_status_bar(term: &mut RawTerminal, status: &Status) -> Result<()> {
    let default_path = OsStr::new("[No Name]");
    let path = status
        .filename
        .and_then(|p| p.file_name())
        .unwrap_or(default_path);
    let dirty_indicator = if status.is_dirty { "(modified)" } else { "" };

    let l_status = format!(
        "{:.20} - {} lines {}",
        path.to_string_lossy(),
        status.lines,
        dirty_indicator
    );
    let r_status = format!(
        "{} | {}/{}",
        status.syntax.filetype,
        status.cursor.y + 1,
        status.lines
    );

    let l_width = cmp::min(l_status.len(), term.screen_size.cols);
    let r_width = cmp::min(r_status.len(), term.screen_size.cols - l_width);
    let sep_width = term.screen_size.cols - l_width - r_width;

    write!(
        term,
        "\x1b[7m{:.wl$}{:ws$}{:.wr$}\x1b[m",
        l_status,
        "",
        r_status,
        wl = l_width,
        ws = sep_width,
        wr = r_width,
    )
    .context(TerminalOutput)?;
    writeln!(term, "\r").context(TerminalOutput)?;
    Ok(())
}

fn draw_message_bar(term: &mut RawTerminal, message: Option<&str>) -> Result<()> {
    write!(term, "\x1b[K").context(TerminalOutput)?;
    if let Some(msg) = message {
        let cols = term.screen_size.cols;
        write!(term, "{:.w$}", msg, w = cols).context(TerminalOutput)?;
    }
    Ok(())
}

pub(crate) fn refresh_screen(
    term: &mut RawTerminal,
    decoder: &mut Decoder,
    editor: &mut Editor,
) -> Result<()> {
    let updated = term.maybe_update_screen_size(decoder).context(Terminal)?;
    if updated {
        let mut render_size = term.screen_size;
        render_size.rows -= 2; // status bar height + message bar height
        editor.set_render_size(render_size);
    }

    let _hide_cursor = term.hide_cursor().context(Terminal)?;

    write!(term, "\x1b[H").context(TerminalOutput)?; // move cursor to top-left corner

    let r = editor.scroll();
    editor.update_status_message();
    editor.update_highlight();

    draw_main(term, editor)?;
    draw_status_bar(term, &editor.status())?;
    draw_message_bar(term, editor.status_message())?;

    write!(term, "\x1b[{};{}H", r.y + 1, r.x + 1).context(TerminalOutput)?; // move cursor

    Ok(())
}

pub(crate) fn flush(term: &mut RawTerminal) -> Result<()> {
    term.flush().context(TerminalOutput)?;

    Ok(())
}
