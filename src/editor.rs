use crate::{
    decode::Decoder,
    frame::Frame,
    geom::{Point, Size},
    input,
    status_message::StatusMessage,
    terminal::RawTerminal,
    text_buffer::TextBuffer,
    text_buffer_view::{self, Status, TextBufferView},
    welcome::{self, Welcome},
};
use itertools::Either;
use std::{
    cell::{Ref, RefMut},
    collections::VecDeque,
    path::{Path, PathBuf},
};

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
    frame: Frame,
    buffer_view: VecDeque<TextBufferView>,
    welcome: Welcome,
    render_size: Size,
    status_message: StatusMessage,
}

impl Editor {
    pub(crate) fn new(render_size: Size) -> Self {
        Editor {
            frame: Frame::new(render_size),
            buffer_view: VecDeque::new(),
            welcome: Welcome::new(render_size),
            render_size,
            status_message: StatusMessage::new(),
        }
    }

    pub(crate) fn open(&mut self, filename: impl Into<PathBuf>) {
        let filename = filename.into();
        match TextBuffer::from_file(filename) {
            Ok(buffer) => {
                if let Some(bv) = self
                    .frame
                    .set_buffer_view(TextBufferView::new(buffer, self.render_size))
                {
                    self.buffer_view.push_back(bv);
                }
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

        let res = self.buffer_mut().unwrap().save();
        match res {
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
        self.frame.buffer_view()
    }

    fn buffer_view_mut(&mut self) -> Option<&mut TextBufferView> {
        self.frame.buffer_view_mut()
    }

    fn buffer_view_or_create(&mut self) -> &mut TextBufferView {
        self.frame.buffer_view_or_create()
    }

    fn buffer(&self) -> Option<Ref<TextBuffer>> {
        self.buffer_view().map(|bv| bv.buffer())
    }

    fn buffer_mut(&mut self) -> Option<RefMut<TextBuffer>> {
        self.buffer_view_mut().map(|bv| bv.buffer_mut())
    }

    pub(crate) fn next_buffer(&mut self) {
        if let Some(bv) = self.buffer_view.pop_front() {
            if let Some(bv) = self.frame.set_buffer_view(bv) {
                self.buffer_view.push_back(bv);
            }
        }
    }

    pub(crate) fn prev_buffer(&mut self) {
        if let Some(bv) = self.buffer_view.pop_back() {
            if let Some(bv) = self.frame.set_buffer_view(bv) {
                self.buffer_view.push_front(bv);
            }
        }
    }

    pub(crate) fn close_buffer(
        &mut self,
        term: &mut RawTerminal,
        decoder: &mut Decoder,
    ) -> input::Result<()> {
        if self.buffer().map(|b| b.dirty()).unwrap_or(false) {
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
        self.frame.close();
        self.next_buffer();
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

    pub(crate) fn status(&self) -> Option<Status> {
        let bv = self.buffer_view()?;
        Some(bv.status())
    }

    pub(crate) fn set_render_size(&mut self, render_size: Size) {
        self.frame.set_render_size(render_size);
        self.welcome.set_render_size(render_size);
        self.render_size = render_size;
    }

    pub(crate) fn render_rows(&self) -> Either<text_buffer_view::RenderRows, welcome::RenderRows> {
        if let Some(bv) = self.buffer_view() {
            Either::Left(bv.render_rows())
        } else {
            Either::Right(self.welcome.render_rows())
        }
    }

    pub(crate) fn scroll(&mut self) -> Point {
        self.frame.scroll()
    }

    pub(crate) fn update_highlight(&mut self) {
        self.frame.update_highlight()
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

    fn is_editable(&self) -> bool {
        self.buffer().map(|b| !b.readonly()).unwrap_or(true)
    }

    pub(crate) fn insert_char(&mut self, ch: char) {
        if !self.is_editable() {
            self.set_status_message("Buffer is readonly");
            return;
        }
        self.buffer_view_or_create().insert_char(ch)
    }

    pub(crate) fn insert_newline(&mut self) {
        if !self.is_editable() {
            self.set_status_message("Buffer is readonly");
            return;
        }
        self.buffer_view_or_create().insert_newline()
    }

    pub(crate) fn delete_back_char(&mut self) {
        if !self.is_editable() {
            self.set_status_message("Buffer is readonly");
            return;
        }
        if let Some(buffer_view) = self.buffer_view_mut() {
            buffer_view.delete_back_char();
        }
    }

    pub(crate) fn delete_char(&mut self) {
        if !self.is_editable() {
            self.set_status_message("Buffer is readonly");
            return;
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
