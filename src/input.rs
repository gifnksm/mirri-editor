use crate::{
    editor::Editor,
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
    match mv {
        Left => {
            if editor.cx > 0 {
                editor.cx -= 1
            }
        }
        Right => {
            if editor.cx + 1 < editor.screen_cols {
                editor.cx += 1
            }
        }
        Up => {
            if editor.cy > 0 {
                editor.cy -= 1
            }
        }
        Down => {
            if editor.cy + 1 < editor.screen_rows {
                editor.cy += 1
            }
        }
    }
}

pub(crate) fn process_keypress(editor: &mut Editor) -> Result<bool> {
    use Key::*;

    if let Some(ch) = editor.term.read_key().context(TerminalError)? {
        if ch == Char(ctrl_key('q')) {
            return Ok(true);
        }

        match ch {
            ArrowUp => move_cursor(editor, CursorMove::Up),
            ArrowDown => move_cursor(editor, CursorMove::Down),
            ArrowLeft => move_cursor(editor, CursorMove::Left),
            ArrowRight => move_cursor(editor, CursorMove::Right),
            Home => editor.cx = 0,
            End => editor.cx = editor.screen_cols.wrapping_sub(1),
            PageUp | PageDown => {
                let mv = if ch == PageUp {
                    CursorMove::Up
                } else {
                    CursorMove::Down
                };
                for _ in 0..editor.screen_rows {
                    move_cursor(editor, mv);
                }
            }
            _ => {}
        }
    }
    Ok(false)
}
