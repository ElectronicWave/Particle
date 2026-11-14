mod app;
mod i18n;
mod pages;
mod states;
mod theme;
mod widgets;

use color_eyre::Result;
use ratail::{App, AppFlow, KeyChord, KeyHint};

use crate::app::hints;
use crate::app::router::AppRouter;
use crate::app::services::Services;
use crate::pages::route::Route;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let services = Services::new()
        .await
        .map_err(|error| color_eyre::eyre::eyre!("{error:#}"))?;
    let mut app = App::new(services, AppRouter, Route::Home);
    app.bind_global(KeyChord::ctrl('q'), KeyHint::new("Ctrl+Q", "Quit"), |_| {
        AppFlow::Exit
    });
    app.set_hint_renderer(hints::render);

    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal).await;
    ratatui::restore();
    result?;

    Ok(())
}
