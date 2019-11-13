use crate::{
    editor::{self, CursorMove, Editor},
    find, output,
    terminal::{self, Input, Key},
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

pub(crate) fn process_keypress(editor: &mut Editor) -> Result<bool> {
    use Key::*;

    if let Some(input) = editor.term.read_input().context(TerminalError)? {
        match dbg!(input) {
            Input {
                key,
                ctrl: true,
                alt: false,
            } => match key {
                Char('M') => editor.insert_newline(), // Ctrl-M : \r
                Char('Q') => {
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
                Char('P') => editor.move_cursor(CursorMove::Up),
                Char('N') => editor.move_cursor(CursorMove::Down),
                Char('B') => editor.move_cursor(CursorMove::Left),
                Char('F') => editor.move_cursor(CursorMove::Right),
                Char('A') => editor.move_cursor(CursorMove::Home),
                Char('E') => editor.move_cursor(CursorMove::End),
                Char('V') => editor.move_cursor(CursorMove::PageDown),
                Char('S') => editor.save()?,
                Char('G') => find::find(editor)?,
                Char('H') => editor.delete_back_char(),
                _ => {}
            },
            Input {
                key,
                ctrl: false,
                alt: true,
            } => match key {
                Char('v') => editor.move_cursor(CursorMove::PageUp),
                _ => {}
            },
            Input {
                key,
                ctrl: false,
                alt: false,
            } => match key {
                ArrowUp => editor.move_cursor(CursorMove::Up),
                ArrowDown => editor.move_cursor(CursorMove::Down),
                ArrowLeft => editor.move_cursor(CursorMove::Left),
                ArrowRight => editor.move_cursor(CursorMove::Right),
                Home => editor.move_cursor(CursorMove::Home),
                End => editor.move_cursor(CursorMove::End),
                PageUp => editor.move_cursor(CursorMove::PageUp),
                PageDown => editor.move_cursor(CursorMove::PageDown),
                Delete => editor.delete_char(),
                Backspace => editor.delete_back_char(),
                Char(ch) => editor.insert_char(ch),
            },
            _ => {}
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

        while let Some(input) = editor.term.read_input().context(TerminalError)? {
            match input {
                Input {
                    key,
                    ctrl: true,
                    alt: false,
                } => match key {
                    Char('H') => {
                        let _ = buf.pop();
                    }
                    Char('M') => {
                        if !buf.is_empty() {
                            editor.set_status_msg("");
                            callback(editor, &mut buf, PromptCommand::Execute);
                            return Ok(Some(buf));
                        }
                    }
                    Char('[') => {
                        editor.set_status_msg("");
                        callback(editor, &mut buf, PromptCommand::Cancel);
                        return Ok(None);
                    }
                    _ => {}
                },
                Input {
                    key,
                    ctrl: false,
                    alt: false,
                } => match key {
                    Delete | Backspace => {
                        let _ = buf.pop();
                    }
                    ArrowLeft | ArrowUp => callback(editor, &mut buf, PromptCommand::FindPrev),
                    ArrowRight | ArrowDown => callback(editor, &mut buf, PromptCommand::FindNext),
                    Char(ch) => {
                        buf.push(ch);
                        callback(editor, &mut buf, PromptCommand::Input);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}
