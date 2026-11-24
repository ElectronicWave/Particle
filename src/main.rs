mod app;
mod drivers;
mod i18n;
mod pages;
mod services;
mod theme;
mod widgets;

use anyhow::Result;
use ratail::{App, AppFlow, KeyChord, KeyHint};

use crate::app::hints;
use crate::app::router::AppRouter;
use crate::pages::route::Route;
use crate::services::Services;

#[tokio::main]
async fn main() -> Result<()> {
    let (config_store, config) = app::config::load().await?;
    i18n::init(config.language.as_deref())?;

    let services = Services::new(config_store, config).await?;
    let mut app = App::new(services, AppRouter, Route::Home);
    app.bind_global(
        KeyChord::ctrl('q'),
        KeyHint::new("Ctrl+Q", t!("common.quit")),
        |_| AppFlow::Exit,
    );
    app.set_hint_renderer(hints::render);
    app.enable_mouse();

    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal).await;
    ratatui::restore();
    result?;

    Ok(())
}
