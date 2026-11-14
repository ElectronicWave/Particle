use std::borrow::Cow;

use crate::t;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Route {
    Home,
    Instances,
    Catalog,
    Downloads,
    Accounts,
    Settings,
}

const ROUTES: [Route; 6] = [
    Route::Home,
    Route::Instances,
    Route::Catalog,
    Route::Downloads,
    Route::Accounts,
    Route::Settings,
];

impl Route {
    pub fn all() -> &'static [Route] {
        &ROUTES
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Home => 0,
            Self::Instances => 1,
            Self::Catalog => 2,
            Self::Downloads => 3,
            Self::Accounts => 4,
            Self::Settings => 5,
        }
    }

    pub fn label(self) -> Cow<'static, str> {
        match self {
            Self::Home => t!("nav.home"),
            Self::Instances => t!("nav.instances"),
            Self::Catalog => t!("nav.catalog"),
            Self::Downloads => t!("nav.downloads"),
            Self::Accounts => t!("nav.accounts"),
            Self::Settings => t!("nav.settings"),
        }
    }

    pub fn next(self) -> Self {
        let next_index = self.index().saturating_add(1).min(ROUTES.len() - 1);
        ROUTES[next_index]
    }

    pub fn previous(self) -> Self {
        let previous_index = self.index().saturating_sub(1);
        ROUTES[previous_index]
    }
}
