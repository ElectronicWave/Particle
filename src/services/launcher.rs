use std::sync::{Arc, RwLock};

use elemental::launcher::Launcher;

#[derive(Clone)]
pub(super) struct LauncherHandle {
    inner: Arc<RwLock<Launcher>>,
}

impl LauncherHandle {
    pub(super) fn new(launcher: Launcher) -> Self {
        Self {
            inner: Arc::new(RwLock::new(launcher)),
        }
    }

    pub(super) fn get(&self) -> Launcher {
        self.inner.read().expect("launcher lock poisoned").clone()
    }

    pub(super) fn replace(&self, launcher: Launcher) {
        *self.inner.write().expect("launcher lock poisoned") = launcher;
    }
}
