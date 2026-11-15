use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use elemental::{
    core::minecraft::MinecraftVersionId,
    driver::{driver::DriverDescriptor, loader_version::LoaderVersionId},
    infra::downloader::DownloadProgress,
    launcher::DriverSpec,
};
use ratail::Redraw;

use super::{
    config::ConfigService, instance::InstanceService, launcher::LauncherHandle,
    runtime::options_for,
};

type SessionStore = Arc<Mutex<BTreeMap<InstallSessionId, InstallSession>>>;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct InstallSessionId(u64);

#[derive(Debug, Clone)]
pub struct InstallSession {
    pub id: InstallSessionId,
    pub instance_name: String,
    pub phase: InstallPhase,
    pub progress: DownloadProgress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallPhase {
    Preparing,
    Done,
    Failed(String),
}

impl InstallPhase {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Preparing)
    }

    pub fn can_dismiss(&self) -> bool {
        !self.is_active()
    }
}

pub struct InstallService {
    launcher: LauncherHandle,
    config: ConfigService,
    instances: InstanceService,
    sessions: SessionStore,
    next_session_id: AtomicU64,
    redraw: Redraw,
}

impl InstallService {
    pub(super) fn new(
        launcher: LauncherHandle,
        config: ConfigService,
        instances: InstanceService,
        redraw: Redraw,
    ) -> Self {
        Self {
            launcher,
            config,
            instances,
            sessions: Arc::new(Mutex::new(BTreeMap::new())),
            next_session_id: AtomicU64::new(0),
            redraw,
        }
    }

    pub fn sessions(&self) -> Vec<InstallSession> {
        self.sessions
            .lock()
            .expect("install sessions lock poisoned")
            .values()
            .rev()
            .cloned()
            .collect()
    }

    pub fn dismiss(&self, id: InstallSessionId) -> bool {
        let removed = {
            let mut sessions = self
                .sessions
                .lock()
                .expect("install sessions lock poisoned");
            match sessions.get(&id) {
                Some(session) if session.phase.can_dismiss() => sessions.remove(&id).is_some(),
                _ => false,
            }
        };
        if removed {
            self.redraw.trigger();
        }
        removed
    }

    pub fn start(
        &self,
        instance_name: String,
        driver: DriverDescriptor,
        game_version: MinecraftVersionId,
        loader_version: Option<LoaderVersionId>,
    ) -> InstallSessionId {
        let id = InstallSessionId(self.next_session_id.fetch_add(1, Ordering::Relaxed));
        let spec = match DriverSpec::from_descriptor(driver, game_version, loader_version) {
            Ok(spec) => spec,
            Err(error) => {
                self.insert_session(InstallSession {
                    id,
                    instance_name,
                    phase: InstallPhase::Failed(error.to_string()),
                    progress: DownloadProgress::default(),
                });
                self.redraw.trigger();
                return id;
            }
        };

        let config = self.config.snapshot();
        let parallelism = config.download_parallelism.max(1);
        let launcher = self.launcher.get();
        let sessions = self.sessions.clone();
        let redraw = self.redraw.clone();

        self.insert_session(InstallSession {
            id,
            instance_name: instance_name.clone(),
            phase: InstallPhase::Preparing,
            progress: DownloadProgress::default(),
        });
        self.redraw.trigger();

        let instances = self.instances.clone();

        tokio::spawn(async move {
            let runtime = match options_for(&config, Some(&instance_name)).await {
                Ok(runtime) => runtime,
                Err(error) => {
                    set_install_failed(&sessions, id, &redraw, format!("{error:#}"));
                    return;
                }
            };

            let session = launcher
                .downloader()
                .session(format!("prepare {instance_name}"))
                .parallelism(parallelism)
                .open()
                .await;
            let session = match session {
                Ok(session) => session,
                Err(error) => {
                    set_install_failed(&sessions, id, &redraw, format!("{error:#}"));
                    return;
                }
            };

            let mut progress = session.progress_receiver();
            let monitor_sessions = sessions.clone();
            let monitor_redraw = redraw.clone();
            let monitor = tokio::spawn(async move {
                while progress.changed().await.is_ok() {
                    let next = progress.borrow_and_update().clone();
                    update_session(&monitor_sessions, id, |session| {
                        session.progress = next;
                    });
                    monitor_redraw.trigger();
                }
            });

            let result = launcher
                .prepare_with_session(session, instance_name, spec, &runtime)
                .await;
            monitor.abort();

            match result {
                Ok(_) => {
                    update_session(&sessions, id, |session| {
                        session.phase = InstallPhase::Done;
                    });
                    redraw.trigger();
                    let _ = instances.load();
                }
                Err(error) => set_install_failed(&sessions, id, &redraw, format!("{error:#}")),
            }
        });

        id
    }

    fn insert_session(&self, session: InstallSession) {
        self.sessions
            .lock()
            .expect("install sessions lock poisoned")
            .insert(session.id, session);
    }
}

fn update_session(
    sessions: &SessionStore,
    id: InstallSessionId,
    update: impl FnOnce(&mut InstallSession),
) {
    if let Ok(mut sessions) = sessions.lock() {
        if let Some(session) = sessions.get_mut(&id) {
            update(session);
        }
    }
}

fn set_install_failed(
    sessions: &SessionStore,
    id: InstallSessionId,
    redraw: &Redraw,
    message: String,
) {
    update_session(sessions, id, |session| {
        session.phase = InstallPhase::Failed(message);
    });
    redraw.trigger();
}
