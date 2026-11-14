use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

use anyhow::{Error, anyhow};
use elemental::{
    core::{
        auth::{
            accounts::{Account, AccountId},
            authorizers::offline::OfflineAuthorizer,
        },
        minecraft::MinecraftVersionId,
        runtime::{
            distribution::Distribution, provider::default_providers,
            providers::custom::new_custom_provider,
        },
    },
    driver::{
        catalog::{GameVersion, LoaderRelease},
        driver::DriverDescriptor,
        loader_version::LoaderVersionId,
    },
    infra::downloader::DownloadProgress,
    launcher::{AccountStore, DriverCatalog, DriverSpec, Instance, LaunchOptions, Launcher},
    shared::scope::Scope,
};
use ratail::{Context, Redraw, Task, TaskCache};

use crate::app::config::{ConfigStore, LauncherConfig, build_launcher, load_instance_config};
use crate::app::launch::{self, LaunchSession};
use crate::pages::contents::catalog::{load_loader_versions, load_versions};
use crate::pages::route::Route;
use crate::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSummary {
    pub id: AccountId,
    pub name: String,
    pub authorizer: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountState {
    pub accounts: Vec<AccountSummary>,
    pub selected: Option<AccountId>,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchReport {
    pub instance_name: String,
    pub executable: PathBuf,
    pub pid: Option<u32>,
    pub log_path: PathBuf,
    pub started_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchOutcome {
    Started(LaunchReport),
    NoAccount,
    InstanceNotFound(String),
}

pub struct Services {
    redraw: Redraw,
    theme: Theme,
    config: Arc<RwLock<LauncherConfig>>,
    config_store: Arc<ConfigStore>,
    config_error: Arc<Mutex<Option<String>>>,
    launcher: Arc<RwLock<Launcher>>,
    accounts: Arc<AccountStore>,
    drivers: Vec<DriverDescriptor>,
    catalogs: TaskCache<&'static str, Vec<GameVersion>, Error>,
    loader_versions: TaskCache<(&'static str, MinecraftVersionId), Vec<LoaderRelease>, Error>,
    account_state: Task<AccountState, Error>,
    instances: Task<Vec<Instance>, Error>,
    runtimes: Task<Vec<Distribution>, Error>,
    launch: Task<LaunchOutcome, Error>,
    install: Arc<Mutex<InstallState>>,
    animation_pending: Arc<AtomicBool>,
}

impl Services {
    pub async fn new(config_store: ConfigStore, config: LauncherConfig) -> anyhow::Result<Self> {
        let redraw = Redraw::new();
        let launcher = build_launcher(&config)?;
        let accounts = Arc::new(AccountStore::open(Scope::Config).await?);
        let drivers = DriverCatalog::supported_drivers().collect();

        Ok(Self {
            redraw: redraw.clone(),
            theme: Theme::dark(),
            config: Arc::new(RwLock::new(config)),
            config_store: Arc::new(config_store),
            config_error: Arc::new(Mutex::new(None)),
            launcher: Arc::new(RwLock::new(launcher)),
            accounts,
            drivers,
            catalogs: TaskCache::new(&redraw),
            loader_versions: TaskCache::new(&redraw),
            account_state: Task::with_redraw(&redraw),
            instances: Task::with_redraw(&redraw),
            runtimes: Task::with_redraw(&redraw),
            launch: Task::with_redraw(&redraw),
            install: Arc::new(Mutex::new(InstallState::default())),
            animation_pending: Arc::new(AtomicBool::new(false)),
        })
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

    pub fn config(&self) -> LauncherConfig {
        self.config.read().expect("config lock poisoned").clone()
    }

    pub fn theme(&self) -> Theme {
        self.theme
    }

    pub fn config_error(&self) -> Option<String> {
        self.config_error
            .lock()
            .expect("config error lock poisoned")
            .clone()
    }

    pub fn launcher(&self) -> Launcher {
        self.launcher
            .read()
            .expect("launcher lock poisoned")
            .clone()
    }

    pub fn drivers(&self) -> &[DriverDescriptor] {
        &self.drivers
    }

    pub fn catalog(&self, driver: DriverDescriptor) -> Task<Vec<GameVersion>, Error> {
        self.catalogs.get_or_create(&driver.id)
    }

    pub fn load_catalog(&self, driver: DriverDescriptor) -> bool {
        self.catalogs
            .spawn_once(driver.id, move |_| load_versions(driver))
    }

    pub fn loader_versions(
        &self,
        driver: DriverDescriptor,
        game: &MinecraftVersionId,
    ) -> Task<Vec<LoaderRelease>, Error> {
        self.loader_versions
            .get_or_create(&(driver.id, game.clone()))
    }

    pub fn load_loader_versions(&self, driver: DriverDescriptor, game: MinecraftVersionId) -> bool {
        let key = (driver.id, game.clone());
        self.loader_versions
            .spawn_once(key, move |_| load_loader_versions(driver, game))
    }

    pub fn account_state(&self) -> Task<AccountState, Error> {
        self.account_state.clone()
    }

    pub fn load_accounts(&self) -> bool {
        let store = self.accounts.clone();
        self.account_state
            .spawn_once(move || async move { load_account_state(&store).await })
    }

    pub fn sign_in_offline(&self, username: String) -> bool {
        let store = self.accounts.clone();
        self.account_state.spawn_once(move || async move {
            let authorizer = OfflineAuthorizer { username };
            let account = store.sign_in(&authorizer).await?;
            store.select(account.account_id()).await?;
            load_account_state(&store).await
        })
    }

    pub fn select_account(&self, account_id: AccountId) -> bool {
        let store = self.accounts.clone();
        self.account_state.spawn_once(move || async move {
            store.select(account_id).await?;
            load_account_state(&store).await
        })
    }

    pub fn remove_account(&self, account_id: AccountId) -> bool {
        let store = self.accounts.clone();
        self.account_state.spawn_once(move || async move {
            store.remove(account_id).await?;
            load_account_state(&store).await
        })
    }

    pub fn instances(&self) -> Task<Vec<Instance>, Error> {
        self.instances.clone()
    }

    pub fn load_instances(&self) -> bool {
        let launcher = self.launcher();
        self.instances.spawn_once(move || async move {
            launcher.inspect_instances().await.map_err(Error::from)
        })
    }

    pub fn runtimes(&self) -> Task<Vec<Distribution>, Error> {
        self.runtimes.clone()
    }

    pub fn load_runtimes(&self) -> bool {
        let custom_paths = self.config().custom_runtime_paths;
        self.runtimes
            .spawn_once(move || async move { detect_runtimes(custom_paths).await })
    }

    pub fn install_state(&self) -> InstallState {
        self.install.lock().expect("install lock poisoned").clone()
    }

    pub fn reset_install(&self) {
        let mut state = self.install.lock().expect("install lock poisoned");
        if !state.phase.is_preparing() {
            *state = InstallState::default();
        }
    }

    pub fn start_install(
        &self,
        instance_name: String,
        driver: DriverDescriptor,
        game_version: MinecraftVersionId,
        loader_version: Option<LoaderVersionId>,
    ) -> bool {
        {
            let state = self.install.lock().expect("install lock poisoned");
            if state.phase.is_preparing() {
                return false;
            }
        }

        let spec = match DriverSpec::from_descriptor(driver, game_version, loader_version) {
            Ok(spec) => spec,
            Err(error) => {
                let mut state = self.install.lock().expect("install lock poisoned");
                state.instance_name = instance_name;
                state.phase = InstallPhase::Failed(error.to_string());
                self.redraw.trigger();
                return true;
            }
        };

        let config = self.config();
        let parallelism = config.download_parallelism.max(1);
        let launcher = self.launcher();
        let install = self.install.clone();
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
        let refresh_launcher = self.launcher();

        tokio::spawn(async move {
            let runtime = match runtime_options_for(&config, Some(&instance_name)).await {
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
                    let _ = instances.spawn_once(move || async move {
                        refresh_launcher
                            .inspect_instances()
                            .await
                            .map_err(Error::from)
                    });
                }
                Err(error) => set_install_failed(&install, &redraw, format!("{error:#}")),
            }
        });

        true
    }

    pub fn launch_task(&self) -> Task<LaunchOutcome, Error> {
        self.launch.clone()
    }

    pub fn launch_instance(&self, instance_name: String) -> bool {
        let store = self.accounts.clone();
        let launcher = self.launcher();
        let config = self.config();
        let redraw = self.redraw.clone();

        self.launch.spawn_once(move || async move {
            let runtime = runtime_options_for(&config, Some(&instance_name)).await?;
            let Some(account) = store.selected().await? else {
                return Ok(LaunchOutcome::NoAccount);
            };
            let credential = account.credential().await?;
            let Some(instance) = launcher.inspect_instance(instance_name.clone()).await? else {
                return Ok(LaunchOutcome::InstanceNotFound(instance_name));
            };
            let prepared = launcher.load_instance(instance).await?;
            let command = launcher
                .build_launch_command(&prepared, &credential, &runtime, &LaunchOptions::default())
                .await?;
            let executable = command.program();
            let session = LaunchSession::create(&config.storage_root, &instance_name, &command)?;
            let child = session.spawn(&command)?;
            let report = LaunchReport {
                instance_name: instance_name.clone(),
                executable,
                pid: child.id(),
                log_path: session.log_path.clone(),
                started_at: session.started_at,
            };
            launch::supervise(child, session, redraw);

            Ok(LaunchOutcome::Started(report))
        })
    }

    pub fn update_config(&self, next: LauncherConfig) -> Result<(), String> {
        let previous = self.config();
        let storage_changed = previous.storage_root != next.storage_root;
        let providers_changed = previous.custom_runtime_paths != next.custom_runtime_paths;

        if storage_changed || providers_changed {
            let launcher = build_launcher(&next).map_err(|error| format!("{error:#}"))?;
            *self.launcher.write().expect("launcher lock poisoned") = launcher;
        }
        *self.config.write().expect("config lock poisoned") = next.clone();
        if providers_changed {
            self.load_runtimes();
        }

        let store = self.config_store.clone();
        let error = self.config_error.clone();
        let redraw = self.redraw.clone();
        let instances = self.instances.clone();
        let launcher = self.launcher();

        tokio::spawn(async move {
            let result = store.save(&next).await.map_err(|error| error.to_string());

            *error.lock().expect("config error lock poisoned") = result.err();
            redraw.trigger();
        });

        if storage_changed {
            instances.reset();
            let _ = self.instances.spawn_once(move || async move {
                launcher.inspect_instances().await.map_err(Error::from)
            });
        }

        Ok(())
    }
}

async fn runtime_options_for(
    config: &LauncherConfig,
    instance_name: Option<&str>,
) -> Result<elemental::launcher::RuntimeOptions, Error> {
    let mut options = config.runtime_options();
    if let Some(path) = &config.java_runtime_path {
        options.runtime_override = Some(resolve_runtime(path.clone()).await?);
    }
    let Some(instance_name) = instance_name else {
        return Ok(options);
    };
    let Some(instance) = load_instance_config(&config.storage_root, instance_name).await? else {
        return Ok(options);
    };

    if let Some(major) = instance.runtime.java_major_version {
        options.runtime_major_version = Some(major);
        if instance.runtime.path.is_none() {
            options.runtime_override = None;
        }
    }
    if let Some(validation) = instance.runtime.validation {
        options.runtime_validation = validation.mode();
    }
    if let Some(path) = instance.runtime.path {
        options.runtime_override = Some(resolve_runtime(path).await?);
    }

    Ok(options)
}

async fn detect_runtimes(
    custom_paths: Vec<std::path::PathBuf>,
) -> Result<Vec<Distribution>, Error> {
    let mut providers = default_providers();
    if !custom_paths.is_empty() {
        let custom = new_custom_provider(custom_paths)
            .map_err(|error| anyhow!("invalid custom runtime path: {error}"))?;
        providers.push(Arc::new(custom));
    }

    Ok(Distribution::from_providers::<Vec<_>>(providers).await)
}

async fn resolve_runtime(path: std::path::PathBuf) -> Result<Distribution, Error> {
    let path = path.canonicalize()?;
    let provider = new_custom_provider(vec![path.clone()])?;
    let distributions = Distribution::from_providers::<Vec<_>>(vec![Arc::new(provider)]).await;
    distributions
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("could not detect a Java runtime at {}", path.display()))
}

async fn load_account_state(store: &AccountStore) -> Result<AccountState, Error> {
    let index = store.index().await;
    let mut accounts = store
        .list()
        .await?
        .iter()
        .map(|account| AccountSummary {
            id: account.account_id(),
            name: account.display_name().to_owned(),
            authorizer: account.authorizer_id().to_owned(),
        })
        .collect::<Vec<AccountSummary>>();
    accounts.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(AccountState {
        accounts,
        selected: index.selected.map(|reference| reference.account_id),
    })
}

fn set_install_failed(install: &Arc<Mutex<InstallState>>, redraw: &Redraw, message: String) {
    if let Ok(mut state) = install.lock() {
        state.phase = InstallPhase::Failed(message);
    }
    redraw.trigger();
}

impl Context for Services {
    type Route = Route;

    fn redraw(&self) -> &Redraw {
        &self.redraw
    }
}
