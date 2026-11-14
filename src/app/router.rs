use ratail::{Page, Router};

use crate::pages::content::Content;
use crate::pages::contents::accounts::AccountsContent;
use crate::pages::contents::catalog::CatalogContent;
use crate::pages::contents::downloads::DownloadsContent;
use crate::pages::contents::home::HomeContent;
use crate::pages::contents::instances::InstancesContent;
use crate::pages::contents::settings::SettingsContent;
use crate::pages::page::ShellPage;
use crate::pages::route::Route;

use super::services::Services;

pub struct AppRouter;

impl Router<Services> for AppRouter {
    fn create(&mut self, route: &Route, _ctx: &Services) -> Box<dyn Page<Services>> {
        match route {
            Route::Home => boxed(HomeContent::default()),
            Route::Instances => boxed(InstancesContent::default()),
            Route::Catalog => boxed(CatalogContent::default()),
            Route::Downloads => boxed(DownloadsContent::default()),
            Route::Accounts => boxed(AccountsContent::default()),
            Route::Settings => boxed(SettingsContent::default()),
        }
    }
}

fn boxed<C>(content: C) -> Box<dyn Page<Services>>
where
    C: Content + 'static,
{
    Box::new(ShellPage::new(content))
}
