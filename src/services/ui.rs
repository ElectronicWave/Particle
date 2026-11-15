use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use ratail::Redraw;

use crate::theme::Theme;

pub struct UiService {
    redraw: Redraw,
    theme: Theme,
    animation_pending: Arc<AtomicBool>,
}

impl UiService {
    pub(super) fn new() -> Self {
        Self {
            redraw: Redraw::new(),
            theme: Theme::dark(),
            animation_pending: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn redraw(&self) -> &Redraw {
        &self.redraw
    }

    pub fn theme(&self) -> Theme {
        self.theme
    }

    /// Requests one repaint after `delay`, coalescing overlapping animation frames.
    pub fn request_frame(&self, delay: Duration) {
        if self.animation_pending.swap(true, Ordering::AcqRel) {
            return;
        }

        let redraw = self.redraw.clone();
        let pending = self.animation_pending.clone();
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            pending.store(false, Ordering::Release);
            redraw.trigger();
        });
    }
}
