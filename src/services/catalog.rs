use anyhow::Error;
use elemental::{
    core::minecraft::{MinecraftVersion, MinecraftVersionId},
    driver::{
        catalog::{GameVersion, LoaderRelease},
        descriptors::VANILLA_DRIVER,
        driver::DriverDescriptor,
    },
};
use ratail::{Redraw, Task, TaskCache};

use super::launcher::LauncherHandle;

pub struct CatalogService {
    launcher: LauncherHandle,
    drivers: Vec<DriverDescriptor>,
    versions: TaskCache<&'static str, Vec<GameVersion>, Error>,
    loader_versions: TaskCache<(&'static str, MinecraftVersionId), Vec<LoaderRelease>, Error>,
}

impl CatalogService {
    pub(super) fn new(launcher: LauncherHandle, redraw: &Redraw) -> Self {
        let mut drivers: Vec<DriverDescriptor> = launcher.get().drivers().descriptors().collect();
        drivers.sort_by_key(|driver| *driver != VANILLA_DRIVER);
        Self {
            launcher,
            drivers,
            versions: TaskCache::new(redraw),
            loader_versions: TaskCache::new(redraw),
        }
    }

    pub fn drivers(&self) -> &[DriverDescriptor] {
        &self.drivers
    }

    pub fn versions(&self, driver: DriverDescriptor) -> Task<Vec<GameVersion>, Error> {
        self.versions.get_or_create(&driver.id)
    }

    pub fn load_versions(&self, driver: DriverDescriptor) -> bool {
        let launcher = self.launcher.clone();
        self.versions
            .spawn_once(driver.id, move |_| load_versions(launcher, driver))
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
        let launcher = self.launcher.clone();
        self.loader_versions
            .spawn_once(key, move |_| load_loader_versions(launcher, driver, game))
    }
}

async fn load_versions(
    launcher: LauncherHandle,
    driver: DriverDescriptor,
) -> Result<Vec<GameVersion>, Error> {
    let launcher = launcher.get();
    let catalog = launcher.drivers().catalog(driver)?;
    let mut versions = catalog.game_versions().await?;

    // Newest first; unparseable ids sort last by raw id.
    versions.sort_by(|left, right| {
        let left_key = MinecraftVersion::parse_id(&left.id)
            .ok()
            .map(|version| version.key());
        let right_key = MinecraftVersion::parse_id(&right.id)
            .ok()
            .map(|version| version.key());
        right_key
            .cmp(&left_key)
            .then_with(|| right.id.as_str().cmp(left.id.as_str()))
    });
    Ok(versions)
}

async fn load_loader_versions(
    launcher: LauncherHandle,
    driver: DriverDescriptor,
    game: MinecraftVersionId,
) -> Result<Vec<LoaderRelease>, Error> {
    let launcher = launcher.get();
    let catalog = launcher.drivers().catalog(driver)?;
    Ok(catalog.loader_versions(&game).await?)
}
