use std::{path::PathBuf, time::SystemTime};

use elemental::{core::auth::accounts::AccountId, infra::downloader::DownloadProgress};

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
