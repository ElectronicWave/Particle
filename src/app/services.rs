use anyhow::Error;
use elemental::{
    core::minecraft::MinecraftVersionId,
    driver::{
        catalog::{GameVersion, LoaderRelease},
        driver::DriverDescriptor,
    },
    launcher::{DriverCatalog, Launcher},
};
use ratail::{Context, Redraw, Task, TaskCache};

use crate::pages::contents::catalog::{load_loader_versions, load_versions};
use crate::pages::route::Route;

pub struct Services {
    redraw: Redraw,
    #[allow(dead_code)]
    launcher: Launcher,
    drivers: Vec<DriverDescriptor>,
    catalogs: TaskCache<&'static str, Vec<GameVersion>, Error>,
    loader_versions: TaskCache<(&'static str, MinecraftVersionId), Vec<LoaderRelease>, Error>,
}

impl Services {
    pub fn new() -> Self {
        let redraw = Redraw::new();
        let launcher = Launcher::builder().build();
        let drivers = DriverCatalog::supported_drivers().collect();
        let catalogs = TaskCache::new(&redraw);
        let loader_versions = TaskCache::new(&redraw);

        Self {
            redraw,
            launcher,
            drivers,
            catalogs,
            loader_versions,
        }
    }

    #[allow(dead_code)]
    pub fn launcher(&self) -> &Launcher {
        &self.launcher
    }

    pub fn drivers(&self) -> &[DriverDescriptor] {
        &self.drivers
    }

    pub fn catalog(&self, driver: DriverDescriptor) -> Task<Vec<GameVersion>, Error> {
        self.catalogs.get_or_create(&driver.id)
    }

    pub fn load_catalog(&self, driver: DriverDescriptor) -> bool {
        self.catalogs
            .spawn_once(driver.id, move |_| load_versions(driver))
    }

    pub fn loader_versions(
        &self,
        driver: DriverDescriptor,
        game: &MinecraftVersionId,
    ) -> Task<Vec<LoaderRelease>, Error> {
        self.loader_versions
            .get_or_create(&(driver.id, game.clone()))
    }

    pub fn load_loader_versions(&self, driver: DriverDescriptor, game: MinecraftVersionId) -> bool {
        let key = (driver.id, game.clone());
        self.loader_versions
            .spawn_once(key, move |_| load_loader_versions(driver, game))
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
