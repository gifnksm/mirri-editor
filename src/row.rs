use crate::syntax::{Syntax, SyntaxState};

#[derive(Debug, Clone)]
pub(crate) struct Row {
    pub(crate) chars: String,
    syntax_state: SyntaxState,
}

impl Row {
    pub(crate) fn new(mut s: String) -> Self {
        s.truncate(s.trim_end_matches(&['\n', '\r'][..]).len());
        Row {
            chars: s,
            syntax_state: SyntaxState::new(),
        }
    }

    pub(crate) fn syntax(&self) -> &SyntaxState {
        &self.syntax_state
    }

    pub(crate) fn syntax_mut(&mut self) -> &mut SyntaxState {
        &mut self.syntax_state
    }

    pub(crate) fn invalidate_syntax(&mut self) {
        self.syntax_state.invalidate();
    }

    pub(crate) fn update_syntax(
        &mut self,
        syntax: &'static Syntax,
        prev_row: Option<&mut Self>,
        next_row: Option<&mut Self>,
    ) {
        self.syntax_state.update(
            &self.chars,
            syntax,
            prev_row.map(|row| &mut row.syntax_state),
            next_row.map(|row| &mut row.syntax_state),
        );
    }

    pub(crate) fn insert_char(&mut self, at: usize, ch: char) {
        self.chars.insert(at, ch);
        self.invalidate_syntax();
    }

    pub(crate) fn delete_char(&mut self, at: usize) {
        self.chars.remove(at);
        self.invalidate_syntax();
    }

    pub(crate) fn append_str(&mut self, s: &str) {
        self.chars.push_str(s.as_ref());
        self.invalidate_syntax();
    }

    pub(crate) fn split(&mut self, at: usize) -> String {
        let out = self.chars.split_off(at);
        if !out.is_empty() {
            self.invalidate_syntax();
        }
        out
    }
}
