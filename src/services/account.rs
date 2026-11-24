use std::sync::{Arc, Mutex};

use anyhow::Error;
use elemental::{
    core::auth::{
        accounts::{Account, AccountId},
        authorizers::{
            microsoft::{MICROSOFT_AUTHORIZER_ID, MicrosoftAuthorizer},
            offline::{OFFLINE_AUTHORIZER_ID, OfflineAuthorizer},
            yggdrasil::{YGGDRASIL_AUTHORIZER_ID, YggdrasilAuthorizer},
        },
        credential::CredentialKind,
    },
    launcher::AccountStore,
};
use ratail::{Redraw, Task};

use crate::{
    services::{CapeService, SkinService},
    t,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSummary {
    pub id: AccountId,
    pub name: String,
    pub authorizer: String,
    pub local: bool,
    pub auth_server: Option<String>,
}

impl AccountSummary {
    pub fn authorizer_label(&self) -> String {
        authorizer_label(&self.authorizer, self.local)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountState {
    pub accounts: Vec<AccountSummary>,
    pub selected: Option<AccountId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AuthorizationState {
    #[default]
    Idle,
    Starting,
    Awaiting {
        verification_uri: String,
        user_code: String,
    },
}

pub struct AccountService {
    store: Arc<AccountStore>,
    skins: Arc<SkinService>,
    capes: Arc<CapeService>,
    state: Task<AccountState, Error>,
    authorization: Arc<Mutex<AuthorizationState>>,
    redraw: Redraw,
}

impl AccountService {
    pub(super) fn new(
        store: Arc<AccountStore>,
        skins: Arc<SkinService>,
        capes: Arc<CapeService>,
        redraw: Redraw,
    ) -> Self {
        Self {
            store,
            skins,
            capes,
            state: Task::with_redraw(&redraw),
            authorization: Arc::new(Mutex::new(AuthorizationState::default())),
            redraw,
        }
    }

    pub fn state(&self) -> Task<AccountState, Error> {
        self.state.clone()
    }

    pub fn is_running(&self) -> bool {
        self.state.is_running()
    }

    pub fn load(&self) -> bool {
        let store = self.store.clone();
        self.state
            .spawn_once(move || async move { load_account_state(&store).await })
    }

    pub fn authorization(&self) -> AuthorizationState {
        self.authorization
            .lock()
            .expect("authorization lock poisoned")
            .clone()
    }

    pub fn sign_in_offline(&self, username: String) -> bool {
        let username = username.trim().to_owned();
        !username.is_empty()
            && self.spawn_sign_in(
                None,
                SignInKind::Offline(OfflineAuthorizer { username }),
                false,
            )
    }

    pub fn sign_in_microsoft(&self, client_id: String) -> bool {
        let client_id = client_id.trim().to_owned();
        if client_id.is_empty() {
            return false;
        }

        let store = self.store.clone();
        let prompt = self.authorization.clone();
        let task_prompt = prompt.clone();
        let redraw = self.redraw.clone();
        let skins = self.skins.clone();
        let capes = self.capes.clone();
        *prompt.lock().expect("authorization lock poisoned") = AuthorizationState::Starting;
        self.redraw.trigger();

        let spawned = self.state.spawn_once(move || async move {
            let handler_prompt = task_prompt.clone();
            let handler_redraw = redraw.clone();
            let authorizer =
                MicrosoftAuthorizer::new(client_id, move |verification_uri, user_code| {
                    *handler_prompt.lock().expect("authorization lock poisoned") =
                        AuthorizationState::Awaiting {
                            verification_uri,
                            user_code,
                        };
                    handler_redraw.trigger();
                    Ok(())
                });

            let result: anyhow::Result<AccountState> = async {
                let account = store.sign_in(&authorizer).await.map_err(Error::from)?;
                finish_sign_in(&store, &skins, &capes, None, &account, true).await
            }
            .await;

            *task_prompt.lock().expect("authorization lock poisoned") = AuthorizationState::Idle;
            redraw.trigger();
            result
        });

        if !spawned {
            *prompt.lock().expect("authorization lock poisoned") = AuthorizationState::Idle;
            self.redraw.trigger();
        }
        spawned
    }

    pub fn sign_in_yggdrasil(&self, base_url: String, username: String, password: String) -> bool {
        let Some(authorizer) = yggdrasil_authorizer(base_url, username, password) else {
            return false;
        };
        self.spawn_sign_in(None, SignInKind::Yggdrasil(authorizer), true)
    }

    pub fn sign_in_local_yggdrasil(&self, username: String) -> bool {
        let username = username.trim().to_owned();
        !username.is_empty()
            && self.spawn_sign_in(
                None,
                SignInKind::Yggdrasil(YggdrasilAuthorizer {
                    base_url: None,
                    username,
                    password: String::new(),
                    client_token: None,
                }),
                false,
            )
    }

    pub fn edit_offline(&self, account_id: AccountId, username: String) -> bool {
        let username = username.trim().to_owned();
        !username.is_empty()
            && self.spawn_sign_in(
                Some(account_id),
                SignInKind::Offline(OfflineAuthorizer { username }),
                false,
            )
    }

    pub fn edit_local_yggdrasil(&self, account_id: AccountId, username: String) -> bool {
        let username = username.trim().to_owned();
        !username.is_empty()
            && self.spawn_sign_in(
                Some(account_id),
                SignInKind::Yggdrasil(YggdrasilAuthorizer {
                    base_url: None,
                    username,
                    password: String::new(),
                    client_token: None,
                }),
                false,
            )
    }

    pub fn edit_yggdrasil(
        &self,
        account_id: AccountId,
        base_url: String,
        username: String,
        password: String,
    ) -> bool {
        let Some(authorizer) = yggdrasil_authorizer(base_url, username, password) else {
            return false;
        };
        self.spawn_sign_in(Some(account_id), SignInKind::Yggdrasil(authorizer), true)
    }

    fn spawn_sign_in(&self, old_id: Option<AccountId>, kind: SignInKind, backup: bool) -> bool {
        let store = self.store.clone();
        let skins = self.skins.clone();
        let capes = self.capes.clone();
        self.state.spawn_once(move || async move {
            match kind {
                SignInKind::Offline(authorizer) => {
                    let account = store.sign_in(&authorizer).await?;
                    finish_sign_in(&store, &skins, &capes, old_id, &account, backup).await
                }
                SignInKind::Yggdrasil(authorizer) => {
                    let account = store.sign_in(&authorizer).await?;
                    finish_sign_in(&store, &skins, &capes, old_id, &account, backup).await
                }
            }
        })
    }

    pub fn cancel_authorization(&self) {
        self.state.cancel();
        *self
            .authorization
            .lock()
            .expect("authorization lock poisoned") = AuthorizationState::Idle;
        self.redraw.trigger();
    }

    pub fn select(&self, account_id: AccountId) -> bool {
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            store.select(account_id).await?;
            load_account_state(&store).await
        })
    }

    pub fn remove(&self, account_id: AccountId) -> bool {
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            store.remove(account_id).await?;
            load_account_state(&store).await
        })
    }
}

enum SignInKind {
    Offline(OfflineAuthorizer),
    Yggdrasil(YggdrasilAuthorizer),
}

async fn finish_sign_in(
    store: &AccountStore,
    skins: &SkinService,
    capes: &CapeService,
    old_id: Option<AccountId>,
    account: &dyn Account,
    backup: bool,
) -> Result<AccountState, Error> {
    let new_id = account.account_id();
    if let Some(old_id) = old_id
        && old_id != new_id
    {
        store.remove(old_id).await?;
    }
    store.select(new_id).await?;
    if backup {
        backup_online_textures(skins, capes, account);
    }
    load_account_state(store).await
}

fn yggdrasil_authorizer(
    base_url: String,
    username: String,
    password: String,
) -> Option<YggdrasilAuthorizer> {
    let base_url = base_url.trim().trim_end_matches('/').to_owned();
    let username = username.trim().to_owned();
    if base_url.is_empty() || username.is_empty() || password.is_empty() {
        return None;
    }
    Some(YggdrasilAuthorizer {
        base_url: Some(base_url),
        username,
        password,
        client_token: None,
    })
}

async fn load_account_state(store: &AccountStore) -> Result<AccountState, Error> {
    let index = store.index().await;
    let mut accounts = Vec::new();
    for account in store.list().await? {
        let credential = account.credential().await?;
        accounts.push(AccountSummary {
            id: account.account_id(),
            name: account.display_name().to_owned(),
            local: account.authorizer_id() == YGGDRASIL_AUTHORIZER_ID
                && credential.kind == CredentialKind::Legacy,
            authorizer: account.authorizer_id().to_owned(),
            auth_server: credential.auth_server,
        });
    }
    accounts.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(AccountState {
        accounts,
        selected: index.selected.map(|reference| reference.account_id),
    })
}

fn backup_online_textures(skins: &SkinService, capes: &CapeService, account: &dyn Account) {
    let name = format!(
        "{}-{}",
        account.display_name(),
        authorizer_slug(account.authorizer_id())
    );
    skins.backup_online(account.account_id(), name.clone());
    if account.authorizer_id() == YGGDRASIL_AUTHORIZER_ID {
        capes.backup_online(account.account_id(), name);
    }
}

fn authorizer_label(authorizer: &str, local: bool) -> String {
    match (authorizer_slug(authorizer), local) {
        ("microsoft", _) => t!("accounts.microsoft").into_owned(),
        ("yggdrasil", true) => t!("accounts.local_yggdrasil").into_owned(),
        ("yggdrasil", false) => t!("accounts.yggdrasil").into_owned(),
        ("offline", _) => t!("accounts.offline").into_owned(),
        (other, _) => other.to_owned(),
    }
}

fn authorizer_slug(authorizer: &str) -> &str {
    match authorizer {
        MICROSOFT_AUTHORIZER_ID => "microsoft",
        YGGDRASIL_AUTHORIZER_ID => "yggdrasil",
        OFFLINE_AUTHORIZER_ID => "offline",
        other => other,
    }
}
