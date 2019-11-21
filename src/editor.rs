use crate::{
    decode::Decoder,
    geom::{Point, Size},
    input,
    render::RenderItem,
    status_message::StatusMessage,
    syntax::Highlight,
    terminal::RawTerminal,
    text_buffer::{Status, TextBuffer},
    text_buffer_view::{self, TextBufferView},
    welcome::Welcome,
};
use itertools::Either;
use std::path::{Path, PathBuf};

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
    buffer_view: Vec<TextBufferView>,
    buffer_idx: usize,
    welcome: Welcome,
    render_size: Size,
    status_message: StatusMessage,
}

impl Editor {
    pub(crate) fn new(render_size: Size) -> Self {
        Editor {
            buffer_view: vec![],
            buffer_idx: 0,
            welcome: Welcome::new(render_size),
            render_size,
            status_message: StatusMessage::new(),
        }
    }

    pub(crate) fn open(&mut self, filename: impl Into<PathBuf>) {
        let filename = filename.into();
        match TextBuffer::from_file(filename) {
            Ok(buffer) => {
                self.buffer_idx = self.buffer_view.len();
                self.buffer_view
                    .push(TextBufferView::new(buffer, self.render_size))
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

    fn buffer_view(&self) -> Option<&TextBufferView> {
        self.buffer_view.get(self.buffer_idx)
    }

    fn buffer_view_mut(&mut self) -> Option<&mut TextBufferView> {
        self.buffer_view.get_mut(self.buffer_idx)
    }

    fn buffer_view_or_create(&mut self) -> &mut TextBufferView {
        if self.buffer_view.is_empty() {
            self.buffer_view
                .push(TextBufferView::new(TextBuffer::new(), self.render_size));
        }
        &mut self.buffer_view[self.buffer_idx]
    }

    fn buffer(&self) -> Option<&TextBuffer> {
        self.buffer_view().map(|bv| bv.buffer())
    }

    fn buffer_mut(&mut self) -> Option<&mut TextBuffer> {
        self.buffer_view_mut().map(|bv| bv.buffer_mut())
    }

    pub(crate) fn next_buffer(&mut self) {
        if !self.buffer_view.is_empty() {
            self.buffer_idx = (self.buffer_idx + 1) % self.buffer_view.len();
        }
    }

    pub(crate) fn prev_buffer(&mut self) {
        if !self.buffer_view.is_empty() {
            if self.buffer_idx == 0 {
                self.buffer_idx = self.buffer_view.len() - 1;
            } else {
                self.buffer_idx -= 1;
            }
        }
    }

    pub(crate) fn close_buffer(
        &mut self,
        term: &mut RawTerminal,
        decoder: &mut Decoder,
    ) -> input::Result<()> {
        if self.buffer_view.is_empty() {
            return Ok(());
        }
        if self.buffer().unwrap().dirty() {
            let prompt = format!(
                "Buffer {} modified; kill anyway? (yes or no) {{}}",
                self.buffer()
                    .unwrap()
                    .filename()
                    .unwrap_or_else(|| Path::new("[no name]"))
                    .display()
            );
            if !input::prompt_confirm(term, decoder, self, &prompt)? {
                return Ok(());
            }
        }
        self.buffer_view.remove(self.buffer_idx);
        if !self.buffer_view.is_empty() {
            self.buffer_idx %= self.buffer_view.len();
        }
        Ok(())
    }

    pub(crate) fn quit(
        &mut self,
        term: &mut RawTerminal,
        decoder: &mut Decoder,
    ) -> input::Result<bool> {
        if self.dirty()
            && !input::prompt_confirm(
                term,
                decoder,
                self,
                "Modified buffers exist; exit anyway? (yes or no) {}",
            )?
        {
            return Ok(false);
        }
        Ok(true)
    }

    pub(crate) fn dirty(&self) -> bool {
        self.buffer_view.iter().any(|b| b.buffer().dirty())
    }

    pub(crate) fn status(&self) -> Status {
        if let Some(buffer_view) = self.buffer_view() {
            buffer_view.status()
        } else {
            self.welcome.status()
        }
    }

    pub(crate) fn set_render_size(&mut self, render_size: Size) {
        if let Some(buffer_view) = self.buffer_view_mut() {
            buffer_view.set_render_size(render_size);
        }
        self.welcome.set_render_size(render_size);
        self.render_size = render_size;
    }

    #[allow(clippy::needless_lifetimes)] // false positive
    pub(crate) fn render_with_highlight<'a>(
        &'a self,
    ) -> impl Iterator<Item = Box<dyn Iterator<Item = (Highlight, RenderItem)> + 'a>> {
        if let Some(buffer_view) = self.buffer_view() {
            Either::Left(buffer_view.render_with_highlight())
        } else {
            Either::Right(self.welcome.render_with_highlight())
        }
    }

    pub(crate) fn scroll(&mut self) -> Point {
        if let Some(buffer_view) = self.buffer_view_mut() {
            buffer_view.scroll()
        } else {
            Point::default()
        }
    }

    pub(crate) fn update_highlight(&mut self) {
        if let Some(buffer) = self.buffer_view_mut() {
            buffer.update_highlight();
        }
    }

    pub(crate) fn status_message(&self) -> Option<&str> {
        self.status_message.message()
    }

    pub(crate) fn set_status_message(&mut self, s: impl Into<String>) {
        self.status_message.set_message(s)
    }

    pub(crate) fn update_status_message(&mut self) {
        self.status_message.update()
    }

    pub(crate) fn move_cursor(&mut self, mv: CursorMove) {
        if let Some(buffer_view) = self.buffer_view_mut() {
            buffer_view.move_cursor(mv)
        }
    }

    pub(crate) fn insert_char(&mut self, ch: char) {
        if let Some(buffer) = self.buffer() {
            if buffer.readonly() {
                self.set_status_message("Buffer is readonly");
                return;
            }
        }
        self.buffer_view_or_create().insert_char(ch)
    }

    pub(crate) fn insert_newline(&mut self) {
        if let Some(buffer) = self.buffer() {
            if buffer.readonly() {
                self.set_status_message("Buffer is readonly");
                return;
            }
        }
        self.buffer_view_or_create().insert_newline()
    }

    pub(crate) fn delete_back_char(&mut self) {
        if let Some(buffer) = self.buffer() {
            if buffer.readonly() {
                self.set_status_message("Buffer is readonly");
                return;
            }
        }
        if let Some(buffer_view) = self.buffer_view_mut() {
            buffer_view.delete_back_char();
        }
    }

    pub(crate) fn delete_char(&mut self) {
        if let Some(buffer) = self.buffer() {
            if buffer.readonly() {
                self.set_status_message("Buffer is readonly");
                return;
            }
        }
        if let Some(buffer_view) = self.buffer_view_mut() {
            buffer_view.delete_char();
        }
    }

    pub(crate) fn find_start(&mut self) -> Option<Find> {
        let buffer_view = self.buffer_view_mut()?;
        Some(Find {
            inner: buffer_view.find_start(),
        })
    }
}

#[derive(Debug)]
pub(crate) struct Find {
    inner: text_buffer_view::Find,
}

impl Find {
    pub(crate) fn execute(&mut self, editor: &mut Editor, query: &str) {
        if let Some(buffer_view) = editor.buffer_view_mut() {
            self.inner.execute(buffer_view, query)
        }
    }
    pub(crate) fn cancel(&mut self, editor: &mut Editor, query: &str) {
        if let Some(buffer_view) = editor.buffer_view_mut() {
            self.inner.cancel(buffer_view, query)
        }
    }
    pub(crate) fn input(&mut self, editor: &mut Editor, query: &str) {
        if let Some(buffer_view) = editor.buffer_view_mut() {
            self.inner.input(buffer_view, query)
        }
    }
    pub(crate) fn search_forward(&mut self, editor: &mut Editor, query: &str) {
        if let Some(buffer_view) = editor.buffer_view_mut() {
            self.inner.search_forward(buffer_view, query)
        }
    }
    pub(crate) fn search_backward(&mut self, editor: &mut Editor, query: &str) {
        if let Some(buffer_view) = editor.buffer_view_mut() {
            self.inner.search_backward(buffer_view, query)
        }
    }
}
