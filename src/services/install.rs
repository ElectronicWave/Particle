use std::sync::{Arc, Mutex};

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

#[derive(Debug, Clone, Default)]
pub struct InstallState {
    pub instance_name: String,
    pub phase: InstallPhase,
    pub progress: DownloadProgress,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum InstallPhase {
    #[default]
    Idle,
    Preparing,
    Done,
    Failed(String),
}

impl InstallPhase {
    pub fn is_preparing(&self) -> bool {
        matches!(self, Self::Preparing)
    }
}

pub struct InstallService {
    launcher: LauncherHandle,
    config: ConfigService,
    instances: InstanceService,
    state: Arc<Mutex<InstallState>>,
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
            state: Arc::new(Mutex::new(InstallState::default())),
            redraw,
        }
    }

    pub fn state(&self) -> InstallState {
        self.state.lock().expect("install lock poisoned").clone()
    }

    pub fn reset(&self) {
        let mut state = self.state.lock().expect("install lock poisoned");
        if !state.phase.is_preparing() {
            *state = InstallState::default();
        }
    }

    pub fn start(
        &self,
        instance_name: String,
        driver: DriverDescriptor,
        game_version: MinecraftVersionId,
        loader_version: Option<LoaderVersionId>,
    ) -> bool {
        {
            let state = self.state.lock().expect("install lock poisoned");
            if state.phase.is_preparing() {
                return false;
            }
        }

        let spec = match DriverSpec::from_descriptor(driver, game_version, loader_version) {
            Ok(spec) => spec,
            Err(error) => {
                let mut state = self.state.lock().expect("install lock poisoned");
                state.instance_name = instance_name;
                state.phase = InstallPhase::Failed(error.to_string());
                self.redraw.trigger();
                return true;
            }
        };

        let config = self.config.snapshot();
        let parallelism = config.download_parallelism.max(1);
        let launcher = self.launcher.get();
        let install = self.state.clone();
        let redraw = self.redraw.clone();

        {
            let mut state = install.lock().expect("install lock poisoned");
            *state = InstallState {
                instance_name: instance_name.clone(),
                phase: InstallPhase::Preparing,
                progress: Default::default(),
            };
        }
        self.redraw.trigger();

        let instances = self.instances.clone();

        tokio::spawn(async move {
            let runtime = match options_for(&config, Some(&instance_name)).await {
                Ok(runtime) => runtime,
                Err(error) => {
                    set_install_failed(&install, &redraw, format!("{error:#}"));
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
                    set_install_failed(&install, &redraw, format!("{error:#}"));
                    return;
                }
            };

            let mut progress = session.progress_receiver();
            let monitor_state = install.clone();
            let monitor_redraw = redraw.clone();
            let monitor = tokio::spawn(async move {
                while progress.changed().await.is_ok() {
                    let next = progress.borrow_and_update().clone();
                    if let Ok(mut state) = monitor_state.lock() {
                        state.progress = next;
                    }
                    monitor_redraw.trigger();
                }
            });

            let result = launcher
                .prepare_with_session(session, instance_name, spec, &runtime)
                .await;
            monitor.abort();

            match result {
                Ok(_) => {
                    if let Ok(mut state) = install.lock() {
                        state.phase = InstallPhase::Done;
                    }
                    redraw.trigger();
                    let _ = instances.load();
                }
                Err(error) => set_install_failed(&install, &redraw, format!("{error:#}")),
            }
        });

        true
    }
}

fn set_install_failed(install: &Arc<Mutex<InstallState>>, redraw: &Redraw, message: String) {
    if let Ok(mut state) = install.lock() {
        state.phase = InstallPhase::Failed(message);
    }
    redraw.trigger();
}
