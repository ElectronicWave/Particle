use std::sync::Arc;

use elemental::{
    launcher::{AccountStore, SkinStore},
    shared::scope::Scope,
};
use ratail::{Context, Redraw};

use crate::app::config::{ConfigStore, LauncherConfig, build_launcher};
use crate::pages::route::Route;

mod account;
mod catalog;
mod config;
mod install;
mod instance;
mod launch;
mod launcher;
mod runtime;
mod skin;
mod ui;

pub use account::{AccountService, AccountState, AuthorizationState};
pub use catalog::CatalogService;
pub use config::ConfigService;
pub use install::{InstallPhase, InstallService, InstallSession, InstallSessionId};
pub use instance::InstanceService;
pub use launch::{LaunchOutcome, LaunchService};
pub use runtime::RuntimeService;
pub use skin::{SkinService, SkinState};
pub use ui::UiService;

use self::launcher::LauncherHandle;

pub struct Services {
    ui: UiService,
    config: ConfigService,
    accounts: AccountService,
    catalog: CatalogService,
    instances: InstanceService,
    runtimes: RuntimeService,
    skins: SkinService,
    install: InstallService,
    launch: LaunchService,
}

impl Services {
    pub async fn new(config_store: ConfigStore, config: LauncherConfig) -> anyhow::Result<Self> {
        let ui = UiService::new();
        let launcher = LauncherHandle::new(build_launcher(&config)?);
        let accounts = Arc::new(AccountStore::open(Scope::Config).await?);
        let skin_store = Arc::new(SkinStore::open(Scope::Config).await?);

        let config_service =
            ConfigService::new(config, config_store, launcher.clone(), ui.redraw().clone());
        let instances = InstanceService::new(launcher.clone(), ui.redraw().clone());
        let runtimes = RuntimeService::new(config_service.clone(), ui.redraw().clone());
        let skins = SkinService::new(skin_store.clone(), accounts.clone(), ui.redraw().clone());
        let install = InstallService::new(
            launcher.clone(),
            config_service.clone(),
            instances.clone(),
            ui.redraw().clone(),
        );
        let launch = LaunchService::new(
            accounts.clone(),
            launcher,
            config_service.clone(),
            skin_store,
            ui.redraw().clone(),
        );
        let account_service = AccountService::new(accounts, ui.redraw().clone());
        let catalog_service = CatalogService::new(ui.redraw());

        Ok(Self {
            ui,
            config: config_service,
            accounts: account_service,
            catalog: catalog_service,
            instances,
            runtimes,
            skins,
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
        &self.skins
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
