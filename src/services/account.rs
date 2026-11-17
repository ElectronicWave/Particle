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

use crate::t;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSummary {
    pub id: AccountId,
    pub name: String,
    pub authorizer: String,
    pub local: bool,
}

impl AccountSummary {
    pub fn authorizer_label(&self) -> String {
        match self.authorizer.as_str() {
            MICROSOFT_AUTHORIZER_ID => t!("accounts.microsoft").into_owned(),
            YGGDRASIL_AUTHORIZER_ID if self.local => t!("accounts.local_yggdrasil").into_owned(),
            YGGDRASIL_AUTHORIZER_ID => t!("accounts.yggdrasil").into_owned(),
            OFFLINE_AUTHORIZER_ID => t!("accounts.offline").into_owned(),
            other => other.to_owned(),
        }
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
    state: Task<AccountState, Error>,
    authorization: Arc<Mutex<AuthorizationState>>,
    redraw: Redraw,
}

impl AccountService {
    pub(super) fn new(store: Arc<AccountStore>, redraw: Redraw) -> Self {
        Self {
            store,
            state: Task::with_redraw(&redraw),
            authorization: Arc::new(Mutex::new(AuthorizationState::default())),
            redraw,
        }
    }

    pub fn state(&self) -> Task<AccountState, Error> {
        self.state.clone()
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
        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            let authorizer = OfflineAuthorizer { username };
            let account = store.sign_in(&authorizer).await?;
            store.select(account.account_id()).await?;
            load_account_state(&store).await
        })
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
                store
                    .select(account.account_id())
                    .await
                    .map_err(Error::from)?;
                load_account_state(&store).await
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
        let base_url = base_url.trim().trim_end_matches('/').to_owned();
        let username = username.trim().to_owned();
        if base_url.is_empty() || username.is_empty() || password.is_empty() {
            return false;
        }

        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            let authorizer = YggdrasilAuthorizer {
                base_url: Some(base_url),
                username,
                password,
                client_token: None,
            };
            let account = store.sign_in(&authorizer).await?;
            store.select(account.account_id()).await?;
            load_account_state(&store).await
        })
    }

    pub fn sign_in_local_yggdrasil(&self, username: String) -> bool {
        let username = username.trim().to_owned();
        if username.is_empty() {
            return false;
        }

        let store = self.store.clone();
        self.state.spawn_once(move || async move {
            let authorizer = YggdrasilAuthorizer {
                base_url: None,
                username,
                password: String::new(),
                client_token: None,
            };
            let account = store.sign_in(&authorizer).await?;
            store.select(account.account_id()).await?;
            load_account_state(&store).await
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
        });
    }
    accounts.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(AccountState {
        accounts,
        selected: index.selected.map(|reference| reference.account_id),
    })
}
