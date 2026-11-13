#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Route {
    Home,
    Instances,
    Catalog,
    Settings,
}

const ROUTES: [Route; 4] = [
    Route::Home,
    Route::Instances,
    Route::Catalog,
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
            Self::Settings => 3,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Instances => "Instances",
            Self::Catalog => "Catalog",
            Self::Settings => "Settings",
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
