use crate::{
    decode::{self, Decoder},
    editor::{CursorMove, Editor},
    find,
    frame::SplitOrientation,
    input::{Input, InputStrExt, Key},
    keymap::KeyMap,
    output,
    terminal::RawTerminal,
};
use snafu::{ResultExt, Snafu};
use std::rc::Rc;

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
                    if editor.quit(term, decoder)? {
                        return Ok(true);
                    }
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
                Char('X') => editor.next_buffer(),
                Char('C') => editor.close_buffer(term, decoder)?,
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
                Char('X') => editor.prev_buffer(),
                Char('2') => editor.split_frame(SplitOrientation::Vertical),
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
    }

    Ok(false)
}

pub(crate) fn default_keymap<'a>(
) -> KeyMap<(&'a mut RawTerminal, &'a mut Decoder, &'a mut Editor), Result<bool>> {
    fn insert(
        km: &mut KeyMap<(&mut RawTerminal, &mut Decoder, &mut Editor), Result<bool>>,
        key: &str,
        act: impl FnMut((&mut RawTerminal, &mut Decoder, &mut Editor)) -> Result<bool> + 'static,
    ) {
        km.insert(key.inputs().map(|i| i.unwrap()), Rc::new(act));
    }

    let mut km = KeyMap::new();
    insert(&mut km, "C-M", |(_, _, editor)| {
        editor.insert_newline();
        Ok(false)
    });
    insert(&mut km, "C-I", |(_, _, editor)| {
        editor.insert_char('\t');
        Ok(false)
    });
    insert(&mut km, "C-?", |(_, _, editor)| {
        editor.delete_back_char();
        Ok(false)
    });
    insert(&mut km, "C-Q", |(term, decoder, editor)| {
        Ok(editor.quit(term, decoder)?)
    });

    let move_cursor = &[
        ("C-P", CursorMove::Up),
        ("C-N", CursorMove::Down),
        ("C-B", CursorMove::Left),
        ("C-F", CursorMove::Right),
        ("C-A", CursorMove::Home),
        ("C-E", CursorMove::End),
        ("C-V", CursorMove::PageDown),
        ("M-v", CursorMove::PageUp),
        ("M-<", CursorMove::BufferHome),
        ("M->", CursorMove::BufferEnd),
        ("<up>", CursorMove::Up),
        ("<down>", CursorMove::Down),
        ("<left>", CursorMove::Left),
        ("<right>", CursorMove::Right),
        ("<home>", CursorMove::Home),
        ("<end>", CursorMove::End),
        ("<page up>", CursorMove::PageUp),
        ("<page down>", CursorMove::PageDown),
    ];
    for (key, mov) in move_cursor {
        let mov = *mov;
        insert(&mut km, key, move |(_, _, editor)| {
            editor.move_cursor(mov);
            Ok(false)
        });
    }

    insert(&mut km, "C-O", |(term, decoder, editor)| {
        editor.open_prompt(term, decoder)?;
        Ok(false)
    });
    insert(&mut km, "C-S", |(term, decoder, editor)| {
        editor.save(term, decoder)?;
        Ok(false)
    });
    insert(&mut km, "C-G", |(term, decoder, editor)| {
        find::find(term, decoder, editor)?;
        Ok(false)
    });
    insert(&mut km, "C-H", |(_, _, editor)| {
        editor.delete_back_char();
        Ok(false)
    });
    insert(&mut km, "C-X", |(_, _, editor)| {
        editor.next_buffer();
        Ok(false)
    });
    insert(&mut km, "M-X", |(_, _, editor)| {
        editor.prev_buffer();
        Ok(false)
    });
    insert(&mut km, "M-2", |(_, _, editor)| {
        editor.split_frame(SplitOrientation::Vertical);
        Ok(false)
    });
    insert(&mut km, "C-C", |(term, decoder, editor)| {
        editor.close_buffer(term, decoder)?;
        Ok(false)
    });
    insert(&mut km, "<delete>", |(_, _, editor)| {
        editor.delete_char();
        Ok(false)
    });

    km
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum PromptCommand {
    Input,
    SearchBackward,
    SearchForward,
    Execute,
    Cancel,
}

pub(crate) fn prompt_confirm(
    term: &mut RawTerminal,
    decoder: &mut Decoder,
    editor: &mut Editor,
    prompt: &str,
) -> Result<bool> {
    if let Some(s) = self::prompt(term, decoder, editor, prompt)? {
        Ok(s.to_lowercase().starts_with('y'))
    } else {
        Ok(false)
    }
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
        output::refresh_screen(term, editor).context(OutputError)?;

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
