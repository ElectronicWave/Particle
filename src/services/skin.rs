use std::{path::PathBuf, sync::Arc, time::Duration};

use anyhow::Error;
use elemental::{
    core::auth::{
        accounts::{Account, AccountId},
        credential::CredentialKind,
        texture::SkinModel,
    },
    launcher::{AccountSkinExt, AccountStore, SelectedSkin, SkinEntry, SkinId, SkinStore},
};
use ratail::{Redraw, Snapshot, Task, TaskCache};

use crate::widgets::{
    png::PngImage,
    skin::{SkinPreview, SkinView},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinState {
    pub account_id: AccountId,
    pub entries: Vec<SkinEntry>,
    pub selected: Option<SelectedSkin>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewSource {
    Profile,
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountPreview {
    pub image: Option<PngImage>,
    pub model: Option<SkinModel>,
    pub source: PreviewSource,
}

pub struct SkinService {
    store: Arc<SkinStore>,
    accounts: Arc<AccountStore>,
    state: Task<SkinState, Error>,
    previews: TaskCache<AccountId, AccountPreview, Error>,
    client: reqwest::Client,
}

impl SkinService {
    pub(super) fn new(store: Arc<SkinStore>, accounts: Arc<AccountStore>, redraw: Redraw) -> Self {
        Self {
            store,
            accounts,
            state: Task::with_redraw(&redraw),
            previews: TaskCache::new(&redraw),
            client: reqwest::Client::new(),
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
        self.previews.remove(&account_id);
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            store.remove(skin_id).await?;
            load_skin_state(&store, account_id).await
        })
    }

    pub fn choose(&self, account_id: AccountId, skin_id: SkinId, model: SkinModel) -> bool {
        self.previews.remove(&account_id);
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
        self.previews.remove(&account_id);
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            store.clear_selection(account_id).await?;
            load_skin_state(&store, account_id).await
        })
    }

    /// Loads the skin an account actually wears.
    ///
    /// Online accounts use their profile textures. Offline and local Yggdrasil
    /// accounts read the local selection, because their textures only exist
    /// while the loopback server runs. When the profile request fails, the
    /// local selection is used as a fallback.
    pub fn preview(&self, account_id: AccountId) -> Task<AccountPreview, Error> {
        let store = self.store.clone();
        let accounts = self.accounts.clone();
        let client = self.client.clone();
        self.previews
            .spawn_once(account_id, move |account_id| async move {
                load_preview(&store, &accounts, &client, account_id).await
            });
        self.previews.get_or_create(&account_id)
    }
}

async fn load_skin_state(store: &SkinStore, account_id: AccountId) -> Result<SkinState, Error> {
    Ok(SkinState {
        account_id,
        entries: store.list().await?,
        selected: store.selected(account_id).await?,
    })
}

async fn load_preview(
    store: &SkinStore,
    accounts: &AccountStore,
    client: &reqwest::Client,
    account_id: AccountId,
) -> Result<AccountPreview, Error> {
    let account = accounts.account(account_id).await?;
    let credential = account.credential().await?;
    if credential.kind != CredentialKind::Legacy {
        match profile_preview(client, account.as_ref()).await {
            Ok(preview) => return Ok(preview),
            Err(error) => {
                if let Some(local) = local_preview(store, account_id).await? {
                    return Ok(local);
                }
                return Err(error);
            }
        }
    }

    Ok(local_preview(store, account_id)
        .await?
        .unwrap_or(AccountPreview {
            image: None,
            model: None,
            source: PreviewSource::Local,
        }))
}

async fn profile_preview(
    client: &reqwest::Client,
    account: &dyn Account,
) -> Result<AccountPreview, Error> {
    let textures = account.textures().await?;
    let Some(skin) = textures.skin else {
        return Ok(AccountPreview {
            image: None,
            model: None,
            source: PreviewSource::Profile,
        });
    };

    let bytes = client
        .get(&skin.url)
        .timeout(Duration::from_secs(15))
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let image = SkinPreview::from_png(&bytes, skin.model, SkinView::Full)?;

    Ok(AccountPreview {
        image: Some(image),
        model: Some(skin.model),
        source: PreviewSource::Profile,
    })
}

async fn local_preview(
    store: &SkinStore,
    account_id: AccountId,
) -> Result<Option<AccountPreview>, Error> {
    let Some(selected) = store.selected(account_id).await? else {
        return Ok(None);
    };

    let bytes = tokio::fs::read(&selected.path).await?;
    let image = SkinPreview::from_png(&bytes, selected.model, SkinView::Full)?;

    Ok(Some(AccountPreview {
        image: Some(image),
        model: Some(selected.model),
        source: PreviewSource::Local,
    }))
}
