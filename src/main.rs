mod client;
mod config;
mod core;
mod daemon;
mod protocol;
mod ui;
mod utils;

use std::cell::RefCell;
use std::env;
use std::process;
use std::rc::Rc;

use gtk4::prelude::*;

use crate::ui::gui::{build_ui, AppState};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "--daemon" {
        daemon::run_daemon();
        return;
    }

    // Client/UI mode
    let app = gtk4::Application::builder()
        .application_id("com.nitrosense.linux")
        .build();

    app.connect_activate(move |app| {
        // AppState::new() now connects to daemon internally
        // We handle connection failure gracefully in UI or here?
        // AppState::new() panics in current gui.rs implementation if connection fails.
        // Ideally we catch it.
        // But AppState::new() returns Self, not Result.
        // Let's rely on its panic or change it later if user complains.
        let state = Rc::new(RefCell::new(AppState::new()));
        let window = build_ui(app, Rc::clone(&state));
        window.present();
    });

    app.run();
}
