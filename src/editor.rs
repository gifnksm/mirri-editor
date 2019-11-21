use crate::{
    decode::Decoder,
    geom::{Point, Size},
    input,
    render::RenderItem,
    syntax::Highlight,
    terminal::RawTerminal,
    text_buffer::{self, Status, TextBuffer},
    welcome::Welcome,
};
use itertools::Either;
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
    BufferHome,
    BufferEnd,
}

#[derive(Debug)]
pub(crate) struct Editor {
    buffer: Vec<TextBuffer>,
    buffer_idx: usize,
    welcome: Welcome,
    render_size: Size,
    pub(crate) quit_times: usize,
    pub(crate) status_msg: Option<(Instant, String)>,
}

impl Editor {
    pub(crate) fn new(render_size: Size) -> Self {
        Editor {
            buffer: vec![],
            buffer_idx: 0,
            welcome: Welcome::new(render_size),
            render_size,
            quit_times: QUIT_TIMES,
            status_msg: None,
        }
    }

    pub(crate) fn open(&mut self, filename: impl Into<PathBuf>) {
        let filename = filename.into();
        match TextBuffer::from_file(filename, self.render_size) {
            Ok(buffer) => {
                self.buffer_idx = self.buffer.len();
                self.buffer.push(buffer)
            }
            Err(e) => self.set_status_message(format!("{}", e)),
        }
    }

    pub(crate) fn open_prompt(
        &mut self,
        term: &mut RawTerminal,
        decoder: &mut Decoder,
    ) -> input::Result<()> {
        if let Some(filename) = input::prompt(term, decoder, self, "Open file: {} (ESC to cancel)")?
        {
            self.open(filename);
        } else {
            self.set_status_message("Open aborted")
        }
        Ok(())
    }

    pub(crate) fn save(
        &mut self,
        term: &mut RawTerminal,
        decoder: &mut Decoder,
    ) -> input::Result<()> {
        if self.buffer().is_none() {
            return Ok(());
        }

        if self.buffer().unwrap().filename().is_none() {
            if let Some(filename) =
                input::prompt(term, decoder, self, "Save as: {} (ESC to cancel)")?.map(Into::into)
            {
                self.buffer_mut().unwrap().set_filename(Some(filename));
            } else {
                self.set_status_message("Save aborted");
                return Ok(());
            }
        }

        match self.buffer_mut().unwrap().save() {
            Ok(bytes) => {
                self.set_status_message(format!("{} bytes written to disk", bytes));
            }
            Err(e) => {
                self.set_status_message(format!("Can't save! {}", e));
            }
        }

        Ok(())
    }

    fn buffer(&self) -> Option<&TextBuffer> {
        self.buffer.get(self.buffer_idx)
    }

    fn buffer_mut(&mut self) -> Option<&mut TextBuffer> {
        self.buffer.get_mut(self.buffer_idx)
    }

    fn buffer_or_create(&mut self) -> &mut TextBuffer {
        if self.buffer.is_empty() {
            self.buffer.push(TextBuffer::new(self.render_size));
        }
        &mut self.buffer[self.buffer_idx]
    }

    pub(crate) fn next_buffer(&mut self) {
        if !self.buffer.is_empty() {
            self.buffer_idx = (self.buffer_idx + 1) % self.buffer.len();
        }
    }

    pub(crate) fn prev_buffer(&mut self) {
        if !self.buffer.is_empty() {
            if self.buffer_idx == 0 {
                self.buffer_idx = self.buffer.len() - 1;
            } else {
                self.buffer_idx -= 1;
            }
        }
    }

    pub(crate) fn dirty(&self) -> bool {
        self.buffer.iter().any(|b| b.dirty())
    }

    pub(crate) fn status(&self) -> Status {
        if let Some(buffer) = self.buffer() {
            buffer.status()
        } else {
            self.welcome.status()
        }
    }

    pub(crate) fn set_render_size(&mut self, render_size: Size) {
        if let Some(buffer) = self.buffer_mut() {
            buffer.set_render_size(render_size);
        }
        self.welcome.set_render_size(render_size);
        self.render_size = render_size;
    }

    #[allow(clippy::needless_lifetimes)] // false positive
    pub(crate) fn render_with_highlight<'a>(
        &'a self,
    ) -> impl Iterator<Item = Box<dyn Iterator<Item = (Highlight, RenderItem)> + 'a>> {
        if let Some(buffer) = self.buffer() {
            Either::Left(buffer.render_with_highlight())
        } else {
            Either::Right(self.welcome.render_with_highlight())
        }
    }

    pub(crate) fn scroll(&mut self) -> Point {
        if let Some(buffer) = self.buffer_mut() {
            buffer.scroll()
        } else {
            Point::default()
        }
    }

    pub(crate) fn update_highlight(&mut self) {
        if let Some(buffer) = self.buffer_mut() {
            buffer.update_highlight();
        }
    }

    pub(crate) fn status_message(&self) -> Option<&str> {
        self.status_msg.as_ref().map(|s| s.1.as_str())
    }

    pub(crate) fn set_status_message(&mut self, s: impl Into<String>) {
        let now = Instant::now();
        self.status_msg = Some((now, s.into()));
    }

    pub(crate) fn update_status_message(&mut self) {
        if let Some((time, _msg)) = &mut self.status_msg {
            if time.elapsed().as_secs() >= 5 {
                self.status_msg = None;
            }
        }
    }

    pub(crate) fn move_cursor(&mut self, mv: CursorMove) {
        if let Some(buffer) = self.buffer_mut() {
            buffer.move_cursor(mv)
        }
    }

    pub(crate) fn insert_char(&mut self, ch: char) {
        if let Some(buffer) = self.buffer() {
            if buffer.readonly() {
                self.set_status_message("Buffer is readonly");
                return;
            }
        }
        self.buffer_or_create().insert_char(ch)
    }

    pub(crate) fn insert_newline(&mut self) {
        if let Some(buffer) = self.buffer() {
            if buffer.readonly() {
                self.set_status_message("Buffer is readonly");
                return;
            }
        }
        self.buffer_or_create().insert_newline()
    }

    pub(crate) fn delete_back_char(&mut self) {
        if let Some(buffer) = self.buffer() {
            if buffer.readonly() {
                self.set_status_message("Buffer is readonly");
                return;
            }
        }
        if let Some(buffer) = self.buffer.get_mut(self.buffer_idx) {
            buffer.delete_back_char();
        }
    }

    pub(crate) fn delete_char(&mut self) {
        if let Some(buffer) = self.buffer() {
            if buffer.readonly() {
                self.set_status_message("Buffer is readonly");
                return;
            }
        }
        if let Some(buffer) = self.buffer.get_mut(self.buffer_idx) {
            buffer.delete_char();
        }
    }

    pub(crate) fn find_start(&mut self) -> Option<Find> {
        let buffer = self.buffer_mut()?;
        Some(Find {
            inner: buffer.find_start(),
        })
    }
}

#[derive(Debug)]
pub(crate) struct Find {
    inner: text_buffer::Find,
}

impl Find {
    pub(crate) fn execute(&mut self, editor: &mut Editor, query: &str) {
        if let Some(buffer) = editor.buffer_mut() {
            self.inner.execute(buffer, query)
        }
    }
    pub(crate) fn cancel(&mut self, editor: &mut Editor, query: &str) {
        if let Some(buffer) = editor.buffer_mut() {
            self.inner.cancel(buffer, query)
        }
    }
    pub(crate) fn input(&mut self, editor: &mut Editor, query: &str) {
        if let Some(buffer) = editor.buffer_mut() {
            self.inner.input(buffer, query)
        }
    }
    pub(crate) fn search_forward(&mut self, editor: &mut Editor, query: &str) {
        if let Some(buffer) = editor.buffer_mut() {
            self.inner.search_forward(buffer, query)
        }
    }
    pub(crate) fn search_backward(&mut self, editor: &mut Editor, query: &str) {
        if let Some(buffer) = editor.buffer_mut() {
            self.inner.search_backward(buffer, query)
        }
    }
}
