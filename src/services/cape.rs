use std::{path::PathBuf, sync::Arc};

use anyhow::Error;
use elemental::{
    core::auth::accounts::AccountId,
    launcher::{CapeEntry, CapeId, CapeStore, SelectedCape},
};
use ratail::{Redraw, Snapshot, Task};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapeState {
    pub account_id: AccountId,
    pub entries: Vec<CapeEntry>,
    pub selected: Option<SelectedCape>,
}

pub struct CapeService {
    store: Arc<CapeStore>,
    state: Task<CapeState, Error>,
}

impl CapeService {
    pub(super) fn new(store: Arc<CapeStore>, redraw: Redraw) -> Self {
        Self {
            store,
            state: Task::with_redraw(&redraw),
        }
    }

    pub fn snapshot(&self) -> Arc<Snapshot<CapeState, Error>> {
        self.state.snapshot()
    }

    pub fn load(&self, account_id: AccountId) -> bool {
        let store = self.store.clone();
        self.state
            .spawn_once(move || async move { load_cape_state(&store, account_id).await })
    }

    pub fn import(&self, account_id: AccountId, name: String, path: PathBuf) -> bool {
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            let bytes = tokio::fs::read(&path).await?;
            store.import(name, bytes).await?;
            load_cape_state(&store, account_id).await
        })
    }

    pub fn rename(&self, account_id: AccountId, cape_id: CapeId, name: String) -> bool {
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            store.rename(cape_id, name).await?;
            load_cape_state(&store, account_id).await
        })
    }

    pub fn remove(&self, account_id: AccountId, cape_id: CapeId) -> bool {
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            store.remove(cape_id).await?;
            load_cape_state(&store, account_id).await
        })
    }

    pub fn choose(&self, account_id: AccountId, cape_id: CapeId) -> bool {
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            store.select(account_id, cape_id).await?;
            load_cape_state(&store, account_id).await
        })
    }

    pub fn clear(&self, account_id: AccountId) -> bool {
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            store.clear_selection(account_id).await?;
            load_cape_state(&store, account_id).await
        })
    }
}

async fn load_cape_state(store: &CapeStore, account_id: AccountId) -> Result<CapeState, Error> {
    Ok(CapeState {
        account_id,
        entries: store.list().await?,
        selected: store.selected(account_id).await?,
    })
}
