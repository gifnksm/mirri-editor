use signal_hook::{SigId, SIGWINCH};
use std::{
    io::Result,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Debug)]
pub(crate) struct SignalReceiver {
    received: Arc<AtomicBool>,
    sigid: SigId,
}

impl SignalReceiver {
    pub(crate) fn new(signal: i32) -> Result<Self> {
        let received = Arc::new(AtomicBool::new(false));
        let sigid = signal_hook::flag::register(signal, received.clone())?;
        Ok(SignalReceiver { received, sigid })
    }

    pub(crate) fn new_sigwinch() -> Result<Self> {
        Self::new(SIGWINCH)
    }

    pub(crate) fn received(&mut self) -> bool {
        self.received.swap(false, Ordering::Relaxed)
    }
}

impl Drop for SignalReceiver {
    fn drop(&mut self) {
        let _ = signal_hook::unregister(self.sigid);
    }
}
