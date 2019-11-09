use std::fmt::Write;

const TAB_STOP: usize = 8;

#[derive(Debug)]
pub(crate) struct Row {
    pub(crate) chars: String,
    pub(crate) render: String,
}

impl Row {
    pub(crate) fn new(mut s: String) -> Self {
        s.truncate(s.trim_end_matches(&['\n', '\r'][..]).len());
        let mut row = Row {
            chars: s,
            render: String::new(),
        };
        row.update();
        row
    }

    pub(crate) fn update(&mut self) {
        self.render.clear();

        let mut is_first = true;
        for s in self.chars.split('\t') {
            if !is_first {
                let width = TAB_STOP - self.render.len() % TAB_STOP;
                let _ = write!(&mut self.render, "{:w$}", "", w = width);
            } else {
                is_first = false;
            }
            self.render.push_str(s);
        }
    }

    pub(crate) fn insert_char(&mut self, at: usize, ch: char) {
        self.chars.insert(at, ch);
        self.update();
    }

    pub(crate) fn cx_to_rx(&self, cx: usize) -> usize {
        let mut rx = 0;
        for ch in self.chars[..cx].bytes() {
            if ch == b'\t' {
                rx += TAB_STOP - (rx % TAB_STOP);
            } else {
                rx += 1;
            }
        }
        rx
    }
}
