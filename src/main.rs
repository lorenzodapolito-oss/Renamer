#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use renamer::ui::{App, APP_TITLE};

fn main() -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title(APP_TITLE)
        .with_inner_size([1100.0, 950.0])
        .with_min_inner_size([900.0, 700.0]);
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    // optional arguments: the folder to open, then the TROVA and SOSTITUISCI texts
    let mut args = std::env::args().skip(1);
    let folder = args.next().unwrap_or_default();
    let find = args.next().unwrap_or_default();
    let replace = args.next().unwrap_or_default();
    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(move |cc| {
            let mut app = App::new(&cc.egui_ctx, folder.clone());
            app.set_texts(&find, &replace);
            Ok(Box::new(app))
        }),
    )
}
