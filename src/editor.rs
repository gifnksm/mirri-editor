use crate::{decode::Decoder, input, terminal::RawTerminal, text_buffer::TextBuffer};
use std::{path::PathBuf, time::Instant};

pub(crate) const QUIT_TIMES: usize = 3;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum CursorMove {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Debug)]
pub(crate) struct Editor {
    pub(crate) buffer: TextBuffer,
    render_rows: usize,
    render_cols: usize,

    pub(crate) quit_times: usize,
    pub(crate) filename: Option<PathBuf>,
    pub(crate) status_msg: Option<(Instant, String)>,
    pub(crate) decoder: Decoder,
    pub(crate) term: RawTerminal,
}

impl Editor {
    pub(crate) fn new(
        decoder: Decoder,
        term: RawTerminal,
        render_rows: usize,
        render_cols: usize,
    ) -> Self {
        Editor {
            buffer: TextBuffer::new(render_rows, render_cols),
            render_cols,
            render_rows,
            quit_times: QUIT_TIMES,
            filename: None,
            status_msg: None,
            decoder,
            term,
        }
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.buffer.is_dirty()
    }

    pub(crate) fn open(&mut self, filename: impl Into<PathBuf>) {
        let filename = filename.into();
        match TextBuffer::from_file(filename, self.render_rows, self.render_cols) {
            Ok(buffer) => self.buffer = buffer,
            Err(e) => self.set_status_msg(format!("{}", e)),
        }
    }

    pub(crate) fn save(&mut self) -> input::Result<()> {
        if self.buffer.filename().is_none() {
            if let Some(filename) =
                input::prompt(self, "Save as: {} (ESC to cancel)")?.map(Into::into)
            {
                self.buffer.set_filename(Some(filename));
            } else {
                self.set_status_msg("Save aborted");
                return Ok(());
            }
        }

        match self.buffer.save() {
            Ok(bytes) => {
                self.set_status_msg(format!("{} bytes written to disk", bytes));
            }
            Err(e) => {
                self.set_status_msg(format!("Can't save! {}", e));
            }
        }

        Ok(())
    }

    pub(crate) fn set_render_size(&mut self, render_rows: usize, render_cols: usize) {
        self.buffer.set_render_size(render_rows, render_cols)
    }

    pub(crate) fn scroll(&mut self) -> (usize, usize) {
        self.buffer.scroll()
    }

    pub(crate) fn set_status_msg(&mut self, s: impl Into<String>) {
        let now = Instant::now();
        self.status_msg = Some((now, s.into()));
    }

    pub(crate) fn move_cursor(&mut self, mv: CursorMove) {
        self.buffer.move_cursor(mv)
    }

    pub(crate) fn insert_char(&mut self, ch: char) {
        self.buffer.insert_char(ch)
    }

    pub(crate) fn insert_newline(&mut self) {
        self.buffer.insert_newline()
    }

    pub(crate) fn delete_back_char(&mut self) {
        self.buffer.delete_back_char()
    }

    pub(crate) fn delete_char(&mut self) {
        self.buffer.delete_char()
    }
}
