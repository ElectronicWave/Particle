use std::{path::PathBuf, sync::Arc};

use anyhow::{Error, anyhow};
use elemental::{
    core::runtime::{
        distribution::Distribution, provider::default_providers,
        providers::custom::new_custom_provider,
    },
    launcher::RuntimeOptions,
};
use ratail::{Redraw, Snapshot, Task};

use crate::app::config::{LauncherConfig, load_instance_config};

use super::config::ConfigService;

pub struct RuntimeService {
    config: ConfigService,
    runtimes: Task<Vec<Distribution>, Error>,
}

impl RuntimeService {
    pub(super) fn new(config: ConfigService, redraw: Redraw) -> Self {
        Self {
            config,
            runtimes: Task::with_redraw(&redraw),
        }
    }

    pub fn snapshot(&self) -> Arc<Snapshot<Vec<Distribution>, Error>> {
        self.runtimes.snapshot()
    }

    pub fn load(&self) -> bool {
        let custom_paths = self.config.snapshot().custom_runtime_paths;
        self.runtimes
            .spawn_once(move || async move { detect_runtimes(custom_paths).await })
    }
}

pub(super) async fn options_for(
    config: &LauncherConfig,
    instance_name: Option<&str>,
) -> Result<RuntimeOptions, Error> {
    let mut options = config.runtime_options();
    if let Some(path) = &config.java_runtime_path {
        options.runtime_override = Some(resolve_runtime(path.clone()).await?);
    }
    let Some(instance_name) = instance_name else {
        return Ok(options);
    };
    let Some(instance) = load_instance_config(&config.storage_root, instance_name).await? else {
        return Ok(options);
    };

    if let Some(major) = instance.runtime.java_major_version {
        options.runtime_major_version = Some(major);
        if instance.runtime.path.is_none() {
            options.runtime_override = None;
        }
    }
    if let Some(validation) = instance.runtime.validation {
        options.runtime_validation = validation.mode();
    }
    if let Some(path) = instance.runtime.path {
        options.runtime_override = Some(resolve_runtime(path).await?);
    }

    Ok(options)
}

async fn detect_runtimes(custom_paths: Vec<PathBuf>) -> Result<Vec<Distribution>, Error> {
    let mut providers = default_providers();
    if !custom_paths.is_empty() {
        let custom = new_custom_provider(custom_paths)
            .map_err(|error| anyhow!("invalid custom runtime path: {error}"))?;
        providers.push(Arc::new(custom));
    }

    Ok(Distribution::from_providers::<Vec<_>>(providers).await)
}

async fn resolve_runtime(path: PathBuf) -> Result<Distribution, Error> {
    let path = path.canonicalize()?;
    let provider = new_custom_provider(vec![path.clone()])?;
    let distributions = Distribution::from_providers::<Vec<_>>(vec![Arc::new(provider)]).await;
    distributions
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("could not detect a Java runtime at {}", path.display()))
}
