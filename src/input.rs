use crate::{
    editor::{self, Editor},
    find, output,
    terminal::{self, Key},
};
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub(crate) enum Error {
    #[snafu(display("{}", source))]
    TerminalError { source: terminal::Error },
    #[snafu(display("{}", source))]
    OutputError { source: output::Error },
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
            if editor.cy + 1 < editor.rows.len() {
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
            Char('\r') => editor.insert_newline(),
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
            Char(ch) if ch == ctrl_key('s') => editor.save()?,
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
            Char(ch) if ch == ctrl_key('f') => find::find(editor)?,
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
            Char(ch) if ch == ctrl_key('l') => {}
            Char('\x1b') => {}
            Char(ch) => editor.insert_char(ch),
        }

        editor.quit_times = editor::QUIT_TIMES;
    }

    Ok(false)
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum PromptCommand {
    Input,
    Execute,
    Cancel,
}

pub(crate) fn prompt(editor: &mut Editor, prompt: &str) -> Result<Option<String>> {
    prompt_with_callback(editor, prompt, |_, _, _| {})
}

pub(crate) fn prompt_with_callback(
    editor: &mut Editor,
    prompt: &str,
    mut callback: impl FnMut(&mut Editor, &mut String, PromptCommand),
) -> Result<Option<String>> {
    use Key::*;

    let mut buf = String::new();
    loop {
        let prompt = prompt.replace("{}", &buf);
        editor.set_status_msg(prompt);
        output::refresh_screen(editor).context(OutputError)?;

        while let Some(key) = editor.term.read_key().context(TerminalError)? {
            match key {
                Char(ch) if ch == ctrl_key('h') => {
                    let _ = buf.pop();
                }
                Delete | Backspace => {
                    let _ = buf.pop();
                }
                Char('\x1b') => {
                    callback(editor, &mut buf, PromptCommand::Cancel);
                    return Ok(None);
                }
                Char('\r') => {
                    if !buf.is_empty() {
                        editor.set_status_msg("");
                        callback(editor, &mut buf, PromptCommand::Execute);
                        return Ok(Some(buf));
                    }
                }
                Char(ch) if !ch.is_control() => {
                    buf.push(ch);
                    callback(editor, &mut buf, PromptCommand::Input);
                }
                _ => {}
            }
        }
    }
}
