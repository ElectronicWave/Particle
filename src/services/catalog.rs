use anyhow::Error;
use elemental::{
    core::minecraft::{MinecraftVersion, MinecraftVersionId},
    driver::{
        catalog::{GameVersion, LoaderRelease},
        driver::DriverDescriptor,
    },
    launcher::DriverCatalog,
};
use ratail::{Redraw, Task, TaskCache};

pub struct CatalogService {
    drivers: Vec<DriverDescriptor>,
    versions: TaskCache<&'static str, Vec<GameVersion>, Error>,
    loader_versions: TaskCache<(&'static str, MinecraftVersionId), Vec<LoaderRelease>, Error>,
}

impl CatalogService {
    pub(super) fn new(redraw: &Redraw) -> Self {
        Self {
            drivers: DriverCatalog::supported_drivers().collect(),
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
        self.versions
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

async fn load_versions(driver: DriverDescriptor) -> Result<Vec<GameVersion>, Error> {
    let catalog = DriverCatalog::for_driver(driver)?;
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
    driver: DriverDescriptor,
    game: MinecraftVersionId,
) -> Result<Vec<LoaderRelease>, Error> {
    let catalog = DriverCatalog::for_driver(driver)?;
    Ok(catalog.loader_versions(&game).await?)
}
