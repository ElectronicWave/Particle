use std::sync::{Arc, Mutex, RwLock};

use ratail::Redraw;

use crate::app::config::{ConfigStore, LauncherConfig, build_launcher};

use super::launcher::LauncherHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct ConfigChange {
    pub(super) storage_root: bool,
    pub(super) runtime_providers: bool,
}

#[derive(Clone)]
pub struct ConfigService {
    config: Arc<RwLock<LauncherConfig>>,
    store: Arc<ConfigStore>,
    error: Arc<Mutex<Option<String>>>,
    launcher: LauncherHandle,
    redraw: Redraw,
}

impl ConfigService {
    pub(super) fn new(
        config: LauncherConfig,
        store: ConfigStore,
        launcher: LauncherHandle,
        redraw: Redraw,
    ) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            store: Arc::new(store),
            error: Arc::new(Mutex::new(None)),
            launcher,
            redraw,
        }
    }

    pub fn snapshot(&self) -> LauncherConfig {
        self.config.read().expect("config lock poisoned").clone()
    }

    pub fn error(&self) -> Option<String> {
        self.error
            .lock()
            .expect("config error lock poisoned")
            .clone()
    }

    pub(super) fn update(&self, next: LauncherConfig) -> Result<ConfigChange, String> {
        let previous = self.snapshot();
        let storage_changed = previous.storage_root != next.storage_root;
        let providers_changed = previous.custom_runtime_paths != next.custom_runtime_paths;

        if storage_changed || providers_changed {
            let launcher = build_launcher(&next).map_err(|error| format!("{error:#}"))?;
            self.launcher.replace(launcher);
        }

        *self.config.write().expect("config lock poisoned") = next.clone();

        let store = self.store.clone();
        let error = self.error.clone();
        let redraw = self.redraw.clone();
        tokio::spawn(async move {
            let result = store.save(&next).await.map_err(|error| error.to_string());

            *error.lock().expect("config error lock poisoned") = result.err();
            redraw.trigger();
        });

        Ok(ConfigChange {
            storage_root: storage_changed,
            runtime_providers: providers_changed,
        })
    }
}
