use std::sync::Arc;

use anyhow::Error;
use elemental::launcher::InspectReport;
use ratail::{Redraw, Snapshot, Task};

use super::launcher::LauncherHandle;

#[derive(Clone)]
pub struct InstanceService {
    launcher: LauncherHandle,
    instances: Task<InspectReport, Error>,
}

impl InstanceService {
    pub(super) fn new(launcher: LauncherHandle, redraw: Redraw) -> Self {
        Self {
            launcher,
            instances: Task::with_redraw(&redraw),
        }
    }

    pub fn snapshot(&self) -> Arc<Snapshot<InspectReport, Error>> {
        self.instances.snapshot()
    }

    pub fn load(&self) -> bool {
        let launcher = self.launcher.get();
        self.instances.spawn_once(move || async move {
            launcher.inspect_instances().await.map_err(Error::from)
        })
    }

    pub(super) fn reset_and_load(&self) -> bool {
        self.instances.reset();
        self.load()
    }
}
