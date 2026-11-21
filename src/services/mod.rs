use std::{
    collections::HashMap,
    future::Future,
    hash::Hash,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use elemental::{
    launcher::{AccountStore, CapeStore, SkinStore},
    shared::scope::Scope,
};
use ratail::{Context, Redraw, SnapshotStatus, Task, TaskCache};

use crate::app::config::{ConfigStore, LauncherConfig, build_launcher};
use crate::pages::route::Route;

mod account;
mod cape;
mod catalog;
mod config;
mod install;
mod instance;
mod launch;
mod launcher;
mod runtime;
mod skin;
mod ui;

pub use account::{AccountService, AccountState, AccountSummary, AuthorizationState};
pub use cape::{CapeAccountKind, CapeService, CapeState, OnlineCape};
pub use catalog::CatalogService;
pub use config::ConfigService;
pub use install::{InstallPhase, InstallService, InstallSession, InstallSessionId};
pub use instance::InstanceService;
pub use launch::{LaunchOutcome, LaunchService};
pub use runtime::RuntimeService;
pub use skin::{AccountPreview, PreviewSource, SkinService, SkinState};
pub use ui::UiService;

use self::launcher::LauncherHandle;

const PREVIEW_RETRY_DELAY: Duration = Duration::from_secs(5);

async fn fetch_texture(client: &reqwest::Client, url: &str) -> anyhow::Result<Vec<u8>> {
    Ok(client
        .get(url)
        .timeout(Duration::from_secs(15))
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?
        .to_vec())
}

struct RetryGate<K>(Mutex<HashMap<K, Instant>>);

impl<K: Eq + Hash + Clone> RetryGate<K> {
    fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    fn allow(&self, key: &K) -> bool {
        let now = Instant::now();
        let mut map = self.0.lock().expect("retry gate lock poisoned");
        map.retain(|_, next| *next > now);
        if map.contains_key(key) {
            return false;
        }
        map.insert(key.clone(), now + PREVIEW_RETRY_DELAY);
        true
    }
}

fn ensure_cached<K, T, E, F, Fut>(
    cache: &TaskCache<K, T, E>,
    retry: &RetryGate<K>,
    key: K,
    make: F,
) -> Task<T, E>
where
    K: Eq + Hash + Clone + Send + 'static,
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, E>> + Send + 'static,
{
    match cache.snapshot(&key).map(|snapshot| snapshot.view().status) {
        Some(SnapshotStatus::Error) => {
            if retry.allow(&key) {
                cache.remove(&key);
                cache.spawn_once(key.clone(), move |_| make());
            }
        }
        None => {
            cache.spawn_once(key.clone(), move |_| make());
        }
        Some(_) => {}
    }
    cache.get_or_create(&key)
}

pub struct Services {
    ui: UiService,
    config: ConfigService,
    accounts: AccountService,
    catalog: CatalogService,
    instances: InstanceService,
    runtimes: RuntimeService,
    skins: Arc<SkinService>,
    capes: Arc<CapeService>,
    install: InstallService,
    launch: LaunchService,
}

impl Services {
    pub async fn new(config_store: ConfigStore, config: LauncherConfig) -> anyhow::Result<Self> {
        let ui = UiService::new();
        let launcher = LauncherHandle::new(build_launcher(&config)?);
        let accounts = Arc::new(AccountStore::open(Scope::Config).await?);
        let skin_store = Arc::new(SkinStore::open(Scope::Config).await?);
        let cape_store = Arc::new(CapeStore::open(Scope::Config).await?);

        let config_service =
            ConfigService::new(config, config_store, launcher.clone(), ui.redraw().clone());
        let instances = InstanceService::new(launcher.clone(), ui.redraw().clone());
        let runtimes = RuntimeService::new(config_service.clone(), ui.redraw().clone());
        let skins = Arc::new(SkinService::new(
            skin_store.clone(),
            accounts.clone(),
            ui.redraw().clone(),
        ));
        let capes = Arc::new(CapeService::new(
            cape_store.clone(),
            accounts.clone(),
            ui.redraw().clone(),
        ));
        let install = InstallService::new(
            launcher.clone(),
            config_service.clone(),
            instances.clone(),
            ui.redraw().clone(),
        );
        let account_service = AccountService::new(
            accounts.clone(),
            skins.clone(),
            capes.clone(),
            ui.redraw().clone(),
        );
        let catalog_service = CatalogService::new(launcher.clone(), ui.redraw());
        let launch = LaunchService::new(
            accounts,
            launcher,
            config_service.clone(),
            skin_store,
            cape_store,
            ui.redraw().clone(),
        );

        Ok(Self {
            ui,
            config: config_service,
            accounts: account_service,
            catalog: catalog_service,
            instances,
            runtimes,
            skins,
            capes,
            install,
            launch,
        })
    }

    pub fn ui(&self) -> &UiService {
        &self.ui
    }

    pub fn config(&self) -> &ConfigService {
        &self.config
    }

    pub fn accounts(&self) -> &AccountService {
        &self.accounts
    }

    pub fn catalog(&self) -> &CatalogService {
        &self.catalog
    }

    pub fn instances(&self) -> &InstanceService {
        &self.instances
    }

    pub fn runtimes(&self) -> &RuntimeService {
        &self.runtimes
    }

    pub fn skins(&self) -> &SkinService {
        self.skins.as_ref()
    }

    pub fn capes(&self) -> &CapeService {
        self.capes.as_ref()
    }

    pub fn install(&self) -> &InstallService {
        &self.install
    }

    pub fn launch(&self) -> &LaunchService {
        &self.launch
    }

    pub fn update_config(&self, next: LauncherConfig) -> Result<(), String> {
        let change = self.config.update(next)?;
        if change.runtime_providers {
            self.runtimes.load();
        }
        if change.storage_root {
            self.instances.reset_and_load();
        }
        Ok(())
    }
}

impl Context for Services {
    type Route = Route;

    fn redraw(&self) -> &Redraw {
        self.ui.redraw()
    }
}
