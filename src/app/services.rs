use anyhow::Error;
use ratail::{Context, Redraw, Task, TaskCache};

use crate::pages::contents::catalog::{CatalogDriver, VersionData, load_versions};
use crate::pages::route::Route;

pub struct Services {
    redraw: Redraw,
    catalog: TaskCache<CatalogDriver, Vec<VersionData>, Error>,
}

impl Services {
    pub fn new() -> Self {
        let redraw = Redraw::new();
        let catalog = TaskCache::new(&redraw);

        Self { redraw, catalog }
    }

    pub fn catalog(&self, driver: CatalogDriver) -> Task<Vec<VersionData>, Error> {
        self.catalog.get_or_create(&driver)
    }

    pub fn load_catalog(&self, driver: CatalogDriver) -> bool {
        self.catalog.spawn_once(driver, load_versions)
    }
}

impl Default for Services {
    fn default() -> Self {
        Self::new()
    }
}

impl Context for Services {
    type Route = Route;

    fn redraw(&self) -> &Redraw {
        &self.redraw
    }
}
