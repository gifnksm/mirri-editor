use std::time::Instant;

#[derive(Debug)]
pub(crate) struct StatusMessage {
    message: Option<(Instant, String)>,
}

impl StatusMessage {
    pub(crate) fn new() -> Self {
        StatusMessage { message: None }
    }

    pub(crate) fn message(&self) -> Option<&str> {
        self.message.as_ref().map(|s| s.1.as_str())
    }

    pub(crate) fn set_message(&mut self, s: impl Into<String>) {
        let now = Instant::now();
        self.message = Some((now, s.into()));
    }

    pub(crate) fn update(&mut self) {
        if let Some((time, _msg)) = &mut self.message {
            if time.elapsed().as_secs() >= 5 {
                self.message = None;
            }
        }
    }
}
