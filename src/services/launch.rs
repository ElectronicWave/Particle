use std::{path::PathBuf, sync::Arc, time::SystemTime};

use anyhow::Error;
use elemental::launcher::{AccountStore, LaunchOptions};
use ratail::{Redraw, Snapshot, Task};

use crate::app::launch::{self, LaunchSession};

use super::{config::ConfigService, launcher::LauncherHandle, runtime::options_for};

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

pub struct LaunchService {
    accounts: Arc<AccountStore>,
    launcher: LauncherHandle,
    config: ConfigService,
    launch: Task<LaunchOutcome, Error>,
    redraw: Redraw,
}

impl LaunchService {
    pub(super) fn new(
        accounts: Arc<AccountStore>,
        launcher: LauncherHandle,
        config: ConfigService,
        redraw: Redraw,
    ) -> Self {
        Self {
            accounts,
            launcher,
            config,
            launch: Task::with_redraw(&redraw),
            redraw,
        }
    }

    pub fn snapshot(&self) -> Arc<Snapshot<LaunchOutcome, Error>> {
        self.launch.snapshot()
    }

    pub fn launch_instance(&self, instance_name: String) -> bool {
        let store = self.accounts.clone();
        let launcher = self.launcher.get();
        let config = self.config.snapshot();
        let redraw = self.redraw.clone();

        self.launch.spawn_once(move || async move {
            let runtime = options_for(&config, Some(&instance_name)).await?;
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
}
