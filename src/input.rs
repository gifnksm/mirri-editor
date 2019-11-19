use crate::{
    decode::{self, Decoder, Input, Key},
    editor::{self, CursorMove, Editor},
    find, output,
    terminal::RawTerminal,
};
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub(crate) enum Error {
    #[snafu(display("{}", source))]
    DecodeError { source: decode::Error },
    #[snafu(display("{}", source))]
    OutputError { source: output::Error },
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn process_keypress(
    term: &mut RawTerminal,
    decoder: &mut Decoder,
    editor: &mut Editor,
) -> Result<bool> {
    use Key::*;

    if let Some(input) = decoder.read_input(term).context(DecodeError)? {
        match input {
            Input {
                key,
                ctrl: true,
                alt: false,
            } => match key {
                Char('M') => editor.insert_newline(),   // Ctrl-M : \r
                Char('I') => editor.insert_char('\t'),  // Ctrl-I : \t
                Char('?') => editor.delete_back_char(), // Ctrl-? : Backspace
                Char('Q') => {
                    if editor.dirty() && editor.quit_times > 0 {
                        editor.set_status_message(format!(
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
                Char('O') => editor.open_prompt(term, decoder)?,
                Char('S') => editor.save(term, decoder)?,
                Char('G') => find::find(term, decoder, editor)?,
                Char('H') => editor.delete_back_char(),
                _ => editor.set_status_message(format!("{} is undefined", input)),
            },
            Input {
                key,
                ctrl: false,
                alt: true,
            } => match key {
                Char('v') => editor.move_cursor(CursorMove::PageUp),
                Char('<') => editor.move_cursor(CursorMove::BufferHome),
                Char('>') => editor.move_cursor(CursorMove::BufferEnd),
                _ => editor.set_status_message(format!("{} is undefined", input)),
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
                Char(ch) => editor.insert_char(ch),
            },
            _ => editor.set_status_message(format!("{} is undefined", input)),
        }

        editor.quit_times = editor::QUIT_TIMES;
    }

    Ok(false)
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum PromptCommand {
    Input,
    SearchBackward,
    SearchForward,
    Execute,
    Cancel,
}

pub(crate) fn prompt(
    term: &mut RawTerminal,
    decoder: &mut Decoder,
    editor: &mut Editor,
    prompt: &str,
) -> Result<Option<String>> {
    prompt_with_callback(term, decoder, editor, prompt, |_, _, _| {})
}

pub(crate) fn prompt_with_callback(
    term: &mut RawTerminal,
    decoder: &mut Decoder,
    editor: &mut Editor,
    prompt: &str,
    mut callback: impl FnMut(&mut Editor, &mut String, PromptCommand),
) -> Result<Option<String>> {
    use Key::*;

    let mut buf = String::new();
    loop {
        let prompt = prompt.replace("{}", &buf);
        editor.set_status_message(prompt);
        output::refresh_screen(term, decoder, editor).context(OutputError)?;

        while let Some(input) = decoder.read_input(term).context(DecodeError)? {
            let cmd = match input {
                Input {
                    key,
                    ctrl: true,
                    alt: false,
                } => match key {
                    Char('H') | Char('?') => {
                        let _ = buf.pop();
                        Some(PromptCommand::Input)
                    }
                    Char('M') => {
                        if !buf.is_empty() {
                            editor.set_status_message("");
                            Some(PromptCommand::Execute)
                        } else {
                            None
                        }
                    }
                    Char('[') => {
                        editor.set_status_message("");
                        Some(PromptCommand::Cancel)
                    }
                    _ => None,
                },
                Input {
                    key,
                    ctrl: false,
                    alt: false,
                } => match key {
                    Delete => {
                        let _ = buf.pop();
                        Some(PromptCommand::Input)
                    }
                    ArrowLeft | ArrowUp => Some(PromptCommand::SearchBackward),
                    ArrowRight | ArrowDown => Some(PromptCommand::SearchForward),
                    Char(ch) => {
                        buf.push(ch);
                        Some(PromptCommand::Input)
                    }
                    _ => None,
                },
                _ => None,
            };

            if let Some(cmd) = cmd {
                callback(editor, &mut buf, cmd);
                match cmd {
                    PromptCommand::Execute => return Ok(Some(buf)),
                    PromptCommand::Cancel => return Ok(None),
                    _ => {}
                }
            }
        }
    }
}
