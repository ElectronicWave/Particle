use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Error;
use elemental::{
    core::auth::{
        accounts::{Account, AccountId},
        authorizers::microsoft::MICROSOFT_AUTHORIZER_ID,
        credential::CredentialKind,
        texture::TextureType,
    },
    launcher::{AccountStore, CapeEntry, CapeId, CapeStore, SelectedCape},
};
use ratail::{Redraw, Snapshot, Task, TaskCache};

use super::{RetryGate, ensure_cached, fetch_texture};
use crate::widgets::{cape::CapePreview, png::PngImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapeAccountKind {
    Local,
    Online,
    Microsoft,
}

impl CapeAccountKind {
    pub fn from_account(authorizer: &str, legacy: bool) -> Self {
        if authorizer == MICROSOFT_AUTHORIZER_ID {
            Self::Microsoft
        } else if legacy {
            Self::Local
        } else {
            Self::Online
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlineCape {
    pub id: Option<String>,
    pub alias: Option<String>,
    pub url: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapeState {
    pub account_id: AccountId,
    pub entries: Vec<CapeEntry>,
    pub selected: Option<SelectedCape>,
    pub kind: CapeAccountKind,
    pub online: Vec<OnlineCape>,
}

impl CapeState {
    pub fn selectable_online(&self) -> impl Iterator<Item = &OnlineCape> {
        self.online.iter().filter(|cape| cape.id.is_some())
    }
}

pub struct CapeService {
    store: Arc<CapeStore>,
    accounts: Arc<AccountStore>,
    state: Task<CapeState, Error>,
    previews: TaskCache<String, PngImage, Error>,
    preview_retry: RetryGate<String>,
    client: reqwest::Client,
}

impl CapeService {
    pub(super) fn new(store: Arc<CapeStore>, accounts: Arc<AccountStore>, redraw: Redraw) -> Self {
        Self {
            store,
            accounts,
            state: Task::with_redraw(&redraw),
            previews: TaskCache::new(&redraw),
            preview_retry: RetryGate::new(),
            client: reqwest::Client::new(),
        }
    }

    pub fn snapshot(&self) -> Arc<Snapshot<CapeState, Error>> {
        self.state.snapshot()
    }

    pub fn load(&self, account_id: AccountId) -> bool {
        let store = self.store.clone();
        let accounts = self.accounts.clone();
        self.state
            .spawn_once(move || async move { load_cape_state(&store, &accounts, account_id).await })
    }

    pub fn import(&self, account_id: AccountId, name: String, path: PathBuf) -> bool {
        let store = self.store.clone();
        let accounts = self.accounts.clone();
        self.state.spawn_once(move || async move {
            let bytes = tokio::fs::read(&path).await?;
            store.import(name, bytes).await?;
            load_cape_state(&store, &accounts, account_id).await
        })
    }

    pub fn rename(&self, account_id: AccountId, cape_id: CapeId, name: String) -> bool {
        let store = self.store.clone();
        let accounts = self.accounts.clone();
        self.state.spawn_once(move || async move {
            store.rename(cape_id, name).await?;
            load_cape_state(&store, &accounts, account_id).await
        })
    }

    pub fn remove(&self, account_id: AccountId, cape_id: CapeId) -> bool {
        let store = self.store.clone();
        let accounts = self.accounts.clone();
        self.state.spawn_once(move || async move {
            store.remove(cape_id).await?;
            load_cape_state(&store, &accounts, account_id).await
        })
    }

    pub fn choose(&self, account_id: AccountId, cape_id: CapeId) -> bool {
        let store = self.store.clone();
        let accounts = self.accounts.clone();
        self.state.spawn_once(move || async move {
            let account = accounts.account(account_id).await?;
            let credential = account.credential().await?;
            match cape_kind(account.authorizer_id(), credential.kind) {
                CapeAccountKind::Microsoft => {}
                CapeAccountKind::Local => store.select(account_id, cape_id).await?,
                CapeAccountKind::Online => {
                    upload_online_cape(&store, account.as_ref(), account_id, cape_id).await?;
                }
            }
            load_cape_state(&store, &accounts, account_id).await
        })
    }

    pub fn select_online(&self, account_id: AccountId, cape_id: Option<String>) -> bool {
        let store = self.store.clone();
        let accounts = self.accounts.clone();
        self.state.spawn_once(move || async move {
            let account = accounts.account(account_id).await?;
            let credential = account.credential().await?;
            if cape_kind(account.authorizer_id(), credential.kind) == CapeAccountKind::Microsoft {
                account.select_cape(cape_id).await?;
            }
            load_cape_state(&store, &accounts, account_id).await
        })
    }

    pub fn backup_online(&self, account_id: AccountId, name: String) -> bool {
        let store = self.store.clone();
        let accounts = self.accounts.clone();
        let client = self.client.clone();
        self.state.spawn_once(move || async move {
            backup_online_cape(&store, &accounts, &client, account_id, name).await?;
            load_cape_state(&store, &accounts, account_id).await
        })
    }

    pub fn clear(&self, account_id: AccountId) -> bool {
        let store = self.store.clone();
        let accounts = self.accounts.clone();
        self.state.spawn_once(move || async move {
            store.clear_selection(account_id).await?;
            load_cape_state(&store, &accounts, account_id).await
        })
    }

    pub fn preview_online(&self, url: &str) -> Task<PngImage, Error> {
        let client = self.client.clone();
        let key = url.to_owned();
        let url = key.clone();
        ensure_cached(
            &self.previews,
            &self.preview_retry,
            key,
            move || async move {
                let bytes = fetch_texture(&client, &url).await?;
                CapePreview::from_png(&bytes)
            },
        )
    }

    pub fn preview_online_snapshot(&self, url: &str) -> Option<Arc<Snapshot<PngImage, Error>>> {
        self.previews.snapshot(&url.to_owned())
    }

    pub fn preview_local(&self, path: &Path) -> Task<PngImage, Error> {
        let key = path.to_string_lossy().into_owned();
        let path = path.to_owned();
        ensure_cached(
            &self.previews,
            &self.preview_retry,
            key,
            move || async move { CapePreview::from_path(&path) },
        )
    }

    pub fn preview_local_snapshot(&self, path: &Path) -> Option<Arc<Snapshot<PngImage, Error>>> {
        self.previews.snapshot(&path.to_string_lossy().into_owned())
    }
}

async fn upload_online_cape(
    store: &CapeStore,
    account: &dyn Account,
    account_id: AccountId,
    cape_id: CapeId,
) -> Result<(), Error> {
    let uploadable = account.textures().await?.uploadable;
    if !uploadable.contains(&TextureType::Cape) {
        anyhow::bail!("account does not allow cape uploads");
    }
    let Some(entry) = store
        .list()
        .await?
        .into_iter()
        .find(|entry| entry.asset.id == cape_id)
    else {
        anyhow::bail!("cape {cape_id} is no longer in the local library");
    };
    let bytes = tokio::fs::read(&entry.path).await?;
    account.upload_cape(bytes).await?;
    store.select(account_id, cape_id).await?;
    Ok(())
}

async fn backup_online_cape(
    store: &CapeStore,
    accounts: &AccountStore,
    client: &reqwest::Client,
    account_id: AccountId,
    name: String,
) -> Result<(), Error> {
    let account = accounts.account(account_id).await?;
    if account.authorizer_id() == MICROSOFT_AUTHORIZER_ID {
        return Ok(());
    }
    let credential = account.credential().await?;
    if credential.kind == CredentialKind::Legacy {
        return Ok(());
    }

    let Some(cape) = account.textures().await?.active_cape().cloned() else {
        return Ok(());
    };

    let name = cape
        .alias
        .filter(|alias| !alias.trim().is_empty())
        .map(|alias| format!("{name}-{alias}"))
        .unwrap_or(name);
    let bytes = fetch_texture(client, &cape.url).await?;
    let asset = store.import(name, bytes).await?;
    store.select(account_id, asset.id).await?;
    Ok(())
}

async fn load_cape_state(
    store: &CapeStore,
    accounts: &AccountStore,
    account_id: AccountId,
) -> Result<CapeState, Error> {
    let account = accounts.account(account_id).await?;
    let credential = account.credential().await?;
    let kind = cape_kind(account.authorizer_id(), credential.kind);
    let online = match kind {
        CapeAccountKind::Microsoft => account
            .textures()
            .await?
            .capes
            .into_iter()
            .map(|cape| OnlineCape {
                id: cape.id,
                alias: cape.alias,
                url: cape.url,
                active: cape.active,
            })
            .collect(),
        CapeAccountKind::Local | CapeAccountKind::Online => Vec::new(),
    };

    Ok(CapeState {
        account_id,
        entries: store.list().await?,
        selected: store.selected(account_id).await?,
        kind,
        online,
    })
}

fn cape_kind(authorizer: &str, credential: CredentialKind) -> CapeAccountKind {
    CapeAccountKind::from_account(authorizer, credential == CredentialKind::Legacy)
}
