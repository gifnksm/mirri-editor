use crate::{editor::Editor, syntax::Highlight, util::SliceExt};
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

fn scroll(editor: &mut Editor) -> usize {
    let rx = if let Some(row) = editor.rows.get(editor.cy) {
        get_render_width(&row.chars[..editor.cx])
    } else {
        0
    };

    if editor.row_off > editor.cy {
        editor.row_off = editor.cy;
    }
    if editor.row_off + (editor.screen_rows - 1) < editor.cy {
        editor.row_off = editor.cy - (editor.screen_rows - 1);
    }
    if editor.col_off > rx {
        editor.col_off = rx;
    }
    if editor.col_off + (editor.screen_cols - 1) < rx {
        editor.col_off = rx - (editor.screen_cols - 1);
    }
    rx
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

fn draw_rows(editor: &mut Editor) -> Result<()> {
    for y in 0..editor.screen_rows {
        let file_row = y + editor.row_off;
        if file_row >= editor.rows.len() {
            if editor.rows.is_empty() && y == editor.screen_rows / 3 {
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
            let [prev, row, next] = editor.rows.get3_mut(file_row);
            let row = row.unwrap();
            row.update_syntax(editor.syntax, prev, next);

            let mut buf = String::new();
            let mut current_col = 0;
            let mut current_color = None;
            for (idx, ch) in row.chars.char_indices() {
                let hl = row.highlight()[idx];
                let (render, width) = render_char(ch, &row.chars[idx..], current_col, &mut buf);

                let (scr_s, scr_e) = (editor.col_off, editor.col_off + editor.screen_cols);
                let (col_s, col_e) = (current_col, current_col + width);
                current_col += width;
                if col_e <= scr_s || col_s >= scr_e {
                    continue;
                }
                if hl == Highlight::Normal {
                    if current_color.is_some() {
                        current_color = None;
                        write!(&mut editor.term, "\x1b[39;49m").context(TerminalOutput)?;
                    }
                } else {
                    let color = hl.to_color();
                    if current_color != Some(color) {
                        current_color = Some(color);
                        write!(&mut editor.term, "\x1b[{};{}m", color.0, color.1)
                            .context(TerminalOutput)?;
                    }
                }
                if col_s < scr_s {
                    let width = col_e - scr_s;
                    write!(&mut editor.term, "{:w$}", "", w = width).context(TerminalOutput)?;
                    continue;
                }
                if col_e > scr_e {
                    let width = scr_e - col_s;
                    write!(&mut editor.term, "{:w$}", "", w = width).context(TerminalOutput)?;
                    continue;
                }
                write!(&mut editor.term, "{}", render).context(TerminalOutput)?;
            }
            if current_color.is_some() {
                write!(&mut editor.term, "\x1b[39;49m").context(TerminalOutput)?;
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

    let rx = scroll(editor);

    draw_rows(editor)?;
    draw_status_bar(editor)?;
    draw_message_bar(editor)?;

    write!(
        &mut editor.term,
        "\x1b[{};{}H",
        (editor.cy - editor.row_off) + 1,
        (rx - editor.col_off) + 1
    )
    .context(TerminalOutput)?; // move cursor
    write!(&mut editor.term, "\x1b[?25h").context(TerminalOutput)?; // show cursor

    Ok(())
}

pub(crate) fn flush(editor: &mut Editor) -> Result<()> {
    editor.term.flush().context(TerminalOutput)?;

    Ok(())
}
