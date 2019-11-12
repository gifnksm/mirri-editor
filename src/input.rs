use crate::{
    editor::{self, CursorMove, Editor},
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
            ArrowUp => editor.move_cursor(CursorMove::Up),
            ArrowDown => editor.move_cursor(CursorMove::Down),
            ArrowLeft => editor.move_cursor(CursorMove::Left),
            ArrowRight => editor.move_cursor(CursorMove::Right),
            Home => editor.move_cursor(CursorMove::Home),
            End => editor.move_cursor(CursorMove::End),
            Char(ch) if ch == ctrl_key('f') => find::find(editor)?,
            Char(ch) if ch == ctrl_key('h') => editor.delete_back_char(),
            Delete => editor.delete_char(),
            Backspace => editor.delete_back_char(),
            PageUp => editor.move_cursor(CursorMove::PageUp),
            PageDown => editor.move_cursor(CursorMove::PageDown),
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
    FindPrev,
    FindNext,
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
                    editor.set_status_msg("");
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
                ArrowLeft | ArrowUp => callback(editor, &mut buf, PromptCommand::FindPrev),
                ArrowRight | ArrowDown => callback(editor, &mut buf, PromptCommand::FindNext),
                Char(ch) if !ch.is_control() => {
                    buf.push(ch);
                    callback(editor, &mut buf, PromptCommand::Input);
                }
                _ => {}
            }
        }
    }
}
