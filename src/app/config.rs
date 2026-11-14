use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::t;
use anyhow::{Context, Result, bail};
use elemental::{
    core::runtime::{
        RuntimeValidationMode, provider::default_providers, providers::custom::CustomProvider,
    },
    launcher::{Launcher, RuntimeOptions},
    shared::scope::Scope,
};
use serde::{Deserialize, Serialize};

pub const DEFAULT_STORAGE_ROOT: &str = ".minecraft";
const CONFIG_ID: &str = "particle";
const CONFIG_DIR: &str = ".elemental";
const CONFIG_EXTENSION: &str = "toml";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeValidation {
    #[default]
    Strict,
    Disabled,
}

impl RuntimeValidation {
    pub fn label(self) -> Cow<'static, str> {
        match self {
            Self::Strict => t!("settings.runtime_strict"),
            Self::Disabled => t!("settings.runtime_disabled"),
        }
    }

    pub const fn toggled(self) -> Self {
        match self {
            Self::Strict => Self::Disabled,
            Self::Disabled => Self::Strict,
        }
    }

    pub const fn mode(self) -> RuntimeValidationMode {
        match self {
            Self::Strict => RuntimeValidationMode::Strict,
            Self::Disabled => RuntimeValidationMode::Disabled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherConfig {
    pub storage_root: PathBuf,
    pub java_runtime_path: Option<PathBuf>,
    pub runtime_validation: RuntimeValidation,
    pub download_parallelism: usize,
    pub custom_runtime_paths: Vec<PathBuf>,
    pub language: Option<String>,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            storage_root: PathBuf::from(DEFAULT_STORAGE_ROOT),
            java_runtime_path: None,
            runtime_validation: RuntimeValidation::Strict,
            download_parallelism: 16,
            custom_runtime_paths: Vec::new(),
            language: None,
        }
    }
}

impl LauncherConfig {
    pub fn runtime_options(&self) -> RuntimeOptions {
        RuntimeOptions {
            runtime_major_version: None,
            runtime_override: None,
            runtime_validation: self.runtime_validation.mode(),
        }
    }

    pub fn config_path(&self) -> PathBuf {
        config_path(&self.storage_root)
    }
}

/// Per-instance settings loaded from `<storage_root>/versions/<name>/.elemental/particle.toml`.
///
/// Every field is optional so an instance only overrides what it needs. Omitted fields
/// inherit the launcher-wide configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InstanceConfig {
    pub runtime: InstanceRuntimeConfig,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InstanceRuntimeConfig {
    pub java_major_version: Option<usize>,
    pub path: Option<PathBuf>,
    pub validation: Option<RuntimeValidation>,
}

pub fn config_path(storage_root: &Path) -> PathBuf {
    storage_root
        .join(CONFIG_DIR)
        .join(CONFIG_ID)
        .with_extension(CONFIG_EXTENSION)
}

/// Validate a user-supplied Java home and return its canonical path.
pub fn validate_custom_runtime(path: &Path) -> Result<PathBuf> {
    CustomProvider::new(vec![path.to_path_buf()])
        .with_context(|| format!("invalid runtime path: {}", path.display()))?;
    path.canonicalize()
        .with_context(|| format!("could not canonicalize runtime path: {}", path.display()))
}

pub fn build_launcher(config: &LauncherConfig) -> Result<Launcher> {
    let mut providers = default_providers();
    if !config.custom_runtime_paths.is_empty() {
        let custom = CustomProvider::new(config.custom_runtime_paths.clone())
            .context("invalid custom runtime path in configuration")?;
        providers.push(Arc::new(custom));
    }

    Ok(Launcher::builder()
        .storage_root(config.storage_root.clone())
        .runtime_providers(providers)
        .build())
}

pub struct ConfigStore;

impl ConfigStore {
    pub async fn save(&self, config: &LauncherConfig) -> Result<()> {
        save_config(config).await
    }
}

pub async fn load() -> Result<(ConfigStore, LauncherConfig)> {
    let default_root = PathBuf::from(DEFAULT_STORAGE_ROOT);
    let bootstrap = read_config(&default_root).await?;
    let legacy = if bootstrap.is_none() {
        read_legacy_config().await?
    } else {
        None
    };
    let seed = bootstrap.or(legacy).unwrap_or_default();

    let config = if seed.storage_root == default_root {
        seed
    } else {
        read_config(&seed.storage_root).await?.unwrap_or(seed)
    };

    save_config(&config).await?;
    Ok((ConfigStore, config))
}

pub async fn load_instance_config(
    storage_root: &Path,
    instance_name: &str,
) -> Result<Option<InstanceConfig>> {
    let root = instance_root(storage_root, instance_name)?;
    let path = config_path(&root);
    if !tokio::fs::try_exists(&path).await? {
        return Ok(None);
    }

    let content = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("failed to read instance config: {}", path.display()))?;
    let config = toml::from_str(&content)
        .with_context(|| format!("failed to parse instance config: {}", path.display()))?;
    Ok(Some(config))
}

async fn save_config(config: &LauncherConfig) -> Result<()> {
    write_config(&config.storage_root, config).await?;

    let default_root = PathBuf::from(DEFAULT_STORAGE_ROOT);
    if config.storage_root != default_root {
        write_config(&default_root, config).await?;
    }

    Ok(())
}

async fn read_config(storage_root: &Path) -> Result<Option<LauncherConfig>> {
    let path = config_path(storage_root);
    if !tokio::fs::try_exists(&path).await? {
        return Ok(None);
    }

    let content = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("failed to read config: {}", path.display()))?;
    let config = toml::from_str(&content)
        .with_context(|| format!("failed to parse config: {}", path.display()))?;
    Ok(Some(config))
}

async fn read_legacy_config() -> Result<Option<LauncherConfig>> {
    let Some(base) = Scope::Config.path() else {
        return Ok(None);
    };
    let path = config_path(&base);
    if !tokio::fs::try_exists(&path).await? {
        return Ok(None);
    }

    let content = tokio::fs::read_to_string(&path)
        .await
        .with_context(|| format!("failed to read legacy config: {}", path.display()))?;
    let profile: LegacyProfile = toml::from_str(&content)
        .with_context(|| format!("failed to parse legacy config: {}", path.display()))?;
    Ok(Some(profile.config))
}

async fn write_config(storage_root: &Path, config: &LauncherConfig) -> Result<()> {
    let path = config_path(storage_root);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create config directory: {}", parent.display()))?;
    }

    let content = toml::to_string_pretty(config).context("failed to serialize launcher config")?;
    tokio::fs::write(&path, content)
        .await
        .with_context(|| format!("failed to write config: {}", path.display()))
}

fn instance_root(storage_root: &Path, instance_name: &str) -> Result<PathBuf> {
    if instance_name.is_empty()
        || instance_name == "."
        || instance_name == ".."
        || instance_name.contains(['/', '\\'])
    {
        bail!("invalid instance name: {instance_name:?}");
    }

    Ok(storage_root.join("versions").join(instance_name))
}

#[derive(Debug, Deserialize)]
struct LegacyProfile {
    #[serde(default)]
    config: LauncherConfig,
}
