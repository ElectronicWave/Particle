use std::borrow::Cow;

use crate::t;
use elemental::{
    core::minecraft::{MinecraftVersion, MinecraftVersionId, MinecraftVersionKind, VersionStage},
    driver::catalog::GameVersion,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum VersionFilter {
    #[default]
    Release,
    Snapshot,
    Legacy,
    All,
}

impl VersionFilter {
    pub(super) const ORDER: [Self; 4] = [Self::Release, Self::Snapshot, Self::Legacy, Self::All];

    pub(super) fn label(self) -> Cow<'static, str> {
        match self {
            Self::Release => t!("catalog.filter.release"),
            Self::Snapshot => t!("catalog.filter.snapshot"),
            Self::Legacy => t!("catalog.filter.legacy"),
            Self::All => t!("catalog.filter.all"),
        }
    }

    pub(super) fn matches(self, version: &GameVersion) -> bool {
        self == Self::All || channel(&version.id) == Some(self)
    }
}

fn channel(id: &MinecraftVersionId) -> Option<VersionFilter> {
    let version = MinecraftVersion::parse_id(id).ok()?;

    Some(match version.kind() {
        MinecraftVersionKind::AlphaBeta(_) => VersionFilter::Legacy,
        MinecraftVersionKind::LegacySnapshot(_) | MinecraftVersionKind::YearSnapshot(_) => {
            VersionFilter::Snapshot
        }
        MinecraftVersionKind::LegacyRelease(release) if release.stage == VersionStage::Release => {
            VersionFilter::Release
        }
        MinecraftVersionKind::YearRelease(release) if release.stage == VersionStage::Release => {
            VersionFilter::Release
        }
        _ => VersionFilter::Snapshot,
    })
}
