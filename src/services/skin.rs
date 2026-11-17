use std::{path::PathBuf, sync::Arc};

use anyhow::Error;
use elemental::{
    core::auth::{accounts::AccountId, credential::CredentialKind, texture::SkinModel},
    launcher::{AccountSkinExt, AccountStore, SelectedSkin, SkinEntry, SkinId, SkinStore},
};
use ratail::{Redraw, Snapshot, Task};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinState {
    pub account_id: AccountId,
    pub entries: Vec<SkinEntry>,
    pub selected: Option<SelectedSkin>,
}

pub struct SkinService {
    store: Arc<SkinStore>,
    accounts: Arc<AccountStore>,
    state: Task<SkinState, Error>,
}

impl SkinService {
    pub(super) fn new(store: Arc<SkinStore>, accounts: Arc<AccountStore>, redraw: Redraw) -> Self {
        Self {
            store,
            accounts,
            state: Task::with_redraw(&redraw),
        }
    }

    pub fn snapshot(&self) -> Arc<Snapshot<SkinState, Error>> {
        self.state.snapshot()
    }

    pub fn load(&self, account_id: AccountId) -> bool {
        let store = self.store.clone();
        self.state
            .spawn_once(move || async move { load_skin_state(&store, account_id).await })
    }

    pub fn import(
        &self,
        account_id: AccountId,
        name: String,
        path: PathBuf,
        model: SkinModel,
    ) -> bool {
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            let bytes = tokio::fs::read(&path).await?;
            store.import(name, bytes, model).await?;
            load_skin_state(&store, account_id).await
        })
    }

    pub fn remove(&self, account_id: AccountId, skin_id: SkinId) -> bool {
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            store.remove(skin_id).await?;
            load_skin_state(&store, account_id).await
        })
    }

    pub fn choose(&self, account_id: AccountId, skin_id: SkinId, model: SkinModel) -> bool {
        let store = self.store.clone();
        let accounts = self.accounts.clone();
        self.state.spawn_once(move || async move {
            let account = accounts.account(account_id).await?;
            let credential = account.credential().await?;
            if credential.kind == CredentialKind::Legacy {
                store.select(account_id, skin_id, model).await?;
            } else {
                AccountSkinExt::skin(account.as_ref(), store.skin(skin_id, model)).await?;
            }
            load_skin_state(&store, account_id).await
        })
    }

    pub fn clear(&self, account_id: AccountId) -> bool {
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            store.clear_selection(account_id).await?;
            load_skin_state(&store, account_id).await
        })
    }
}

async fn load_skin_state(store: &SkinStore, account_id: AccountId) -> Result<SkinState, Error> {
    Ok(SkinState {
        account_id,
        entries: store.list().await?,
        selected: store.selected(account_id).await?,
    })
}
