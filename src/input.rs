use crate::{
    editor::{self, Editor},
    file,
    terminal::{self, Key},
};
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub(crate) enum Error {
    #[snafu(display("{}", source))]
    TerminalError { source: terminal::Error },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

fn ctrl_key(b: char) -> char {
    debug_assert!(b.is_ascii_lowercase());
    ((b as u8) & 0x1f) as char
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum CursorMove {
    Up,
    Down,
    Left,
    Right,
}

fn move_cursor(editor: &mut Editor, mv: CursorMove) {
    use CursorMove::*;
    let row = editor.rows.get(editor.cy);
    match mv {
        Left => {
            if editor.cx > 0 {
                editor.cx -= 1
            } else if editor.cy > 0 {
                editor.cy -= 1;
                editor.cx = editor.rows[editor.cy].chars.len();
            }
        }
        Right => {
            if let Some(row) = row {
                if editor.cx < row.chars.len() {
                    editor.cx += 1
                } else {
                    editor.cy += 1;
                    editor.cx = 0;
                }
            }
        }
        Up => {
            if editor.cy > 0 {
                editor.cy -= 1
            }
        }
        Down => {
            if editor.cy < editor.rows.len() {
                editor.cy += 1
            }
        }
    }

    let row = editor.rows.get(editor.cy);
    let row_len = row.map(|r| r.chars.len()).unwrap_or(0);
    if editor.cx > row_len {
        editor.cx = row_len;
    }
}

pub(crate) fn process_keypress(editor: &mut Editor) -> Result<bool> {
    use Key::*;

    if let Some(ch) = editor.term.read_key().context(TerminalError)? {
        match ch {
            Char('\r') => {} // TODO
            Char(ch) if ch == ctrl_key('q') => {
                if editor.dirty && editor.quit_times > 0 {
                    editor.set_status_msg(format!(
                        "WARNING!!! File has changed. Press Ctrl-Q {} more times to quit.",
                        editor.quit_times
                    ));
                    editor.quit_times -= 1;
                    return Ok(false);
                }
                return Ok(true);
            }
            Char(ch) if ch == ctrl_key('s') => match file::save(editor) {
                Ok(bytes) => {
                    editor.set_status_msg(format!("{} bytes written to disk", bytes));
                }
                Err(e) => {
                    editor.set_status_msg(format!("Can't save! {}", e));
                }
            },
            ArrowUp => move_cursor(editor, CursorMove::Up),
            ArrowDown => move_cursor(editor, CursorMove::Down),
            ArrowLeft => move_cursor(editor, CursorMove::Left),
            ArrowRight => move_cursor(editor, CursorMove::Right),
            Home => editor.cx = 0,
            End => {
                if let Some(row) = editor.rows.get(editor.cy) {
                    editor.cx = row.chars.len()
                }
            }
            Char(ch) if ch == ctrl_key('h') => editor.delete_char(),
            Delete => {
                move_cursor(editor, CursorMove::Right);
                editor.delete_char();
            }
            Backspace => editor.delete_char(),
            PageUp | PageDown => {
                let mv = if ch == PageUp {
                    editor.cy = editor.row_off;
                    CursorMove::Up
                } else {
                    editor.cy = editor.row_off + editor.screen_rows - 1;
                    if editor.cy > editor.rows.len() {
                        editor.cy = editor.rows.len();
                    }
                    CursorMove::Down
                };
                for _ in 0..editor.screen_rows {
                    move_cursor(editor, mv);
                }
            }
            Char(ch) if ch == ctrl_key('l') => {} //TODO
            Char('\x1b') => {}                    //TODO
            Char(ch) => editor.insert_char(ch),
        }

        editor.quit_times = editor::QUIT_TIMES;
    }

    Ok(false)
}
