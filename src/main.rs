//! DiagramCraft entry point: CLI parsing and native window startup.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod document;
mod svg;

use craftkit_ui::cli::{self, CliAction};
use craftkit_ui::{WindowOptions, run_native};

fn main() -> craftkit_ui::eframe::Result<()> {
    let action = match cli::parse_args(std::env::args().skip(1)) {
        Ok(action) => action,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    let options = match action {
        CliAction::Help => {
            print!(
                "{}",
                cli::help_text(
                    "diagramcraft",
                    "DiagramCraft — diagram editor with shapes, connectors and SVG export"
                )
            );
            return Ok(());
        }
        CliAction::Version => {
            println!("DiagramCraft {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        CliAction::Run(options) => options,
    };

    run_native(WindowOptions::new("DiagramCraft"), move |context| {
        let app = app::DiagramApp::new(context, options);
        Ok(Box::new(app) as Box<dyn craftkit_ui::eframe::App>)
    })
}
