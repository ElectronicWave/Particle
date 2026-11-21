use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Error;
use elemental::{
    core::auth::{
        accounts::{Account, AccountId},
        credential::CredentialKind,
        texture::{SkinModel, TextureType},
    },
    launcher::{AccountSkinExt, AccountStore, SelectedSkin, SkinEntry, SkinId, SkinStore},
};
use ratail::{Redraw, Snapshot, Task, TaskCache};

use crate::widgets::{
    png::PngImage,
    skin::{SkinPreview, SkinView},
};

use super::{RetryGate, ensure_cached, fetch_texture};

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
    /// Texture kinds the account advertises as uploadable.
    ///
    /// `None` means the profile could not be read, so callers should not
    /// treat the account as read-only.
    pub uploadable: Option<Vec<TextureType>>,
}

pub struct SkinService {
    store: Arc<SkinStore>,
    accounts: Arc<AccountStore>,
    state: Task<SkinState, Error>,
    previews: TaskCache<AccountId, AccountPreview, Error>,
    textures: TaskCache<SkinTextureKey, PngImage, Error>,
    preview_retry: RetryGate<AccountId>,
    texture_retry: RetryGate<SkinTextureKey>,
    client: reqwest::Client,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SkinTextureKey(PathBuf, bool, bool);

fn texture_key(path: &Path, model: SkinModel, view: SkinView) -> SkinTextureKey {
    SkinTextureKey(
        path.to_owned(),
        model == SkinModel::Slim,
        view == SkinView::Full,
    )
}

impl SkinService {
    pub(super) fn new(store: Arc<SkinStore>, accounts: Arc<AccountStore>, redraw: Redraw) -> Self {
        Self {
            store,
            accounts,
            state: Task::with_redraw(&redraw),
            previews: TaskCache::new(&redraw),
            textures: TaskCache::new(&redraw),
            preview_retry: RetryGate::new(),
            texture_retry: RetryGate::new(),
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
                let uploadable = account.textures().await?.uploadable;
                if !uploadable.contains(&TextureType::Skin) {
                    anyhow::bail!("account does not allow skin uploads");
                }
                AccountSkinExt::skin(account.as_ref(), store.skin(skin_id, model)).await?;
            }
            load_skin_state(&store, account_id).await
        })
    }

    pub fn set_model(&self, account_id: AccountId, skin_id: SkinId, model: SkinModel) -> bool {
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            store.set_model(skin_id, model).await?;
            load_skin_state(&store, account_id).await
        })
    }

    pub fn backup_online(&self, account_id: AccountId, name: String) -> bool {
        let store = self.store.clone();
        let accounts = self.accounts.clone();
        let client = self.client.clone();
        self.state.spawn_once(move || async move {
            backup_online_skin(&store, &accounts, &client, account_id, name).await?;
            load_skin_state(&store, account_id).await
        })
    }

    pub fn rename(&self, account_id: AccountId, skin_id: SkinId, name: String) -> bool {
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            store.rename(skin_id, name).await?;
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
        ensure_cached(
            &self.previews,
            &self.preview_retry,
            account_id,
            move || async move { load_preview(&store, &accounts, &client, account_id).await },
        )
    }

    pub fn preview_snapshot(
        &self,
        account_id: AccountId,
    ) -> Option<Arc<Snapshot<AccountPreview, Error>>> {
        self.previews.snapshot(&account_id)
    }

    pub fn preview_texture(
        &self,
        path: &Path,
        model: SkinModel,
        view: SkinView,
    ) -> Task<PngImage, Error> {
        let key = texture_key(path, model, view);
        let path = path.to_owned();
        ensure_cached(
            &self.textures,
            &self.texture_retry,
            key,
            move || async move { SkinPreview::from_path(&path, model, view) },
        )
    }

    pub fn preview_texture_snapshot(
        &self,
        path: &Path,
        model: SkinModel,
        view: SkinView,
    ) -> Option<Arc<Snapshot<PngImage, Error>>> {
        self.textures.snapshot(&texture_key(path, model, view))
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
            uploadable: None,
        }))
}

async fn profile_preview(
    client: &reqwest::Client,
    account: &dyn Account,
) -> Result<AccountPreview, Error> {
    let textures = account.textures().await?;
    let uploadable = Some(textures.uploadable);
    let Some(skin) = textures.skin else {
        return Ok(AccountPreview {
            image: None,
            model: None,
            source: PreviewSource::Profile,
            uploadable,
        });
    };

    let bytes = fetch_texture(client, &skin.url).await?;
    let image = SkinPreview::from_png(&bytes, skin.model, SkinView::Head)?;

    Ok(AccountPreview {
        image: Some(image),
        model: Some(skin.model),
        source: PreviewSource::Profile,
        uploadable,
    })
}

async fn backup_online_skin(
    store: &SkinStore,
    accounts: &AccountStore,
    client: &reqwest::Client,
    account_id: AccountId,
    name: String,
) -> Result<(), Error> {
    let account = accounts.account(account_id).await?;
    let credential = account.credential().await?;
    if credential.kind == CredentialKind::Legacy {
        return Ok(());
    }

    let Some(skin) = account.textures().await?.skin else {
        return Ok(());
    };

    let bytes = fetch_texture(client, &skin.url).await?;
    store.import(name, bytes, skin.model).await?;
    Ok(())
}

async fn local_preview(
    store: &SkinStore,
    account_id: AccountId,
) -> Result<Option<AccountPreview>, Error> {
    let Some(selected) = store.selected(account_id).await? else {
        return Ok(None);
    };

    let bytes = tokio::fs::read(&selected.path).await?;
    let image = SkinPreview::from_png(&bytes, selected.model, SkinView::Head)?;

    Ok(Some(AccountPreview {
        image: Some(image),
        model: Some(selected.model),
        source: PreviewSource::Local,
        uploadable: None,
    }))
}
