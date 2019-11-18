use crate::{
    decode::Decoder,
    editor::Editor,
    syntax::Highlight,
    terminal::{self, RawTerminal},
    text_buffer::TextBuffer,
    util::SliceExt,
};
use snafu::{Backtrace, ResultExt, Snafu};
use std::{
    cmp,
    ffi::OsStr,
    fmt::Write as _,
    io::{self, Write},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const TAB_STOP: usize = 8;

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

pub(crate) fn get_render_width(s: &str) -> usize {
    let mut buf = String::new();
    let mut cur_col = 0;
    for (idx, ch) in s.char_indices() {
        let (_, width) = render_char(ch, &s[idx..], cur_col, &mut buf);
        cur_col += width;
    }
    cur_col
}

pub(crate) fn get_cx_from_rx(s: &str, rx: usize) -> usize {
    let mut buf = String::new();
    let mut cur_col = 0;
    for (idx, ch) in s.char_indices() {
        if rx == cur_col {
            return idx;
        }
        let (_, width) = render_char(ch, &s[idx..], cur_col, &mut buf);
        if cur_col + width > rx {
            return idx;
        }
        cur_col += width;
    }
    s.len()
}

fn render_char<'a>(
    ch: char,
    chars: &'a str,
    cur_col: usize,
    buf: &'a mut String,
) -> (&'a str, usize) {
    if ch == '\t' {
        let width = TAB_STOP - cur_col % TAB_STOP;
        buf.clear();
        write!(buf, "{:w$}", "", w = width).unwrap();
        return (buf, width);
    }
    if ch.is_ascii_control() {
        let val = ((ch as u8) + b'@') as char;
        buf.clear();
        write!(buf, "^{}", val).unwrap();
        return (buf, 2);
    }
    if ch.is_control() {
        buf.clear();
        write!(buf, "{}", ch.escape_debug()).unwrap(); // TODO: write appropriate representation
        return (buf, buf.width()); // TODO: select width_cjk() or width()
    }
    let len = ch.len_utf8();
    (&chars[..len], ch.width().unwrap()) // TODO: select width_cjk() or width()
}

fn draw_main(term: &mut RawTerminal, editor: &mut Editor) -> Result<()> {
    let buffer = &mut editor.buffer;
    if buffer.rows.is_empty() {
        draw_welcome(term, editor)?;
    } else {
        draw_rows(term, buffer)?;
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

fn draw_rows(term: &mut RawTerminal, buffer: &mut TextBuffer) -> Result<()> {
    // update syntax before drawing
    let max_file_row = buffer.render_rect.size.rows + buffer.render_rect.origin.y;
    for y in 0..max_file_row {
        let [prev, row, next] = buffer.rows.get3_mut(y);
        if let Some(row) = row {
            row.update_syntax(buffer.syntax, prev, next);
        }
    }

    // rendering
    for y in 0..buffer.render_rect.size.rows {
        let file_row = y + buffer.render_rect.origin.y;
        if let Some(row) = buffer.rows.get(file_row) {
            let (scr_s, scr_e) = (
                buffer.render_rect.origin.x,
                buffer.render_rect.origin.x + buffer.render_rect.size.cols,
            );

            let mut buf = String::new();
            let mut current_col = 0;
            let mut current_color = None;
            for (idx, ch) in row.chars.char_indices() {
                let col_s = current_col;
                if col_s >= scr_e {
                    break;
                }

                let (render, width) = render_char(ch, &row.chars[idx..], current_col, &mut buf);
                let col_e = current_col + width;
                current_col += width;
                if col_e <= scr_s {
                    continue;
                }

                let hl = row.syntax().highlight_at(idx);
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
                if col_s < scr_s {
                    let width = col_e - scr_s;
                    write!(term, "{:w$}", "", w = width).context(TerminalOutput)?;
                    continue;
                }
                if col_e > scr_e {
                    let width = scr_e - col_s;
                    write!(term, "{:w$}", "", w = width).context(TerminalOutput)?;
                    continue;
                }
                write!(term, "{}", render).context(TerminalOutput)?;
            }
            if current_color.is_some() {
                write!(term, "\x1b[39;49m").context(TerminalOutput)?;
            }
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

fn draw_status_bar(term: &mut RawTerminal, editor: &mut Editor) -> Result<()> {
    let default_path = OsStr::new("[No Name]");
    let path = editor
        .filename
        .as_ref()
        .and_then(|p| p.file_name())
        .unwrap_or(default_path);
    let dirty_indicator = if editor.is_dirty() { "(modified)" } else { "" };

    let l_status = format!(
        "{:.20} - {} lines {}",
        path.to_string_lossy(),
        editor.buffer.rows.len(),
        dirty_indicator
    );
    let r_status = format!(
        "{} | {}/{}",
        editor.buffer.syntax.filetype,
        editor.buffer.c.y + 1,
        editor.buffer.rows.len()
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

fn draw_message_bar(term: &mut RawTerminal, editor: &mut Editor) -> Result<()> {
    write!(term, "\x1b[K").context(TerminalOutput)?;
    if let Some((time, msg)) = &mut editor.status_msg {
        if time.elapsed().as_secs() < 5 {
            let cols = term.screen_size.cols;
            write!(term, "{:.w$}", msg, w = cols).context(TerminalOutput)?;
        } else {
            editor.status_msg = None;
        }
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
    write!(term, "\x1b[?25l").context(TerminalOutput)?; // hide cursor
    write!(term, "\x1b[H").context(TerminalOutput)?; // move cursor to top-left corner

    let r = editor.scroll();

    draw_main(term, editor)?;
    draw_status_bar(term, editor)?;
    draw_message_bar(term, editor)?;

    write!(term, "\x1b[{};{}H", r.y + 1, r.x + 1).context(TerminalOutput)?; // move cursor
    write!(term, "\x1b[?25h").context(TerminalOutput)?; // show cursor

    Ok(())
}

pub(crate) fn flush(term: &mut RawTerminal) -> Result<()> {
    term.flush().context(TerminalOutput)?;

    Ok(())
}
