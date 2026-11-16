pub mod actions;
pub mod app;
pub mod input;
pub mod refresh;
pub mod render;
pub mod run;
pub mod system;
pub mod views;
pub mod widgets;

pub use app::Tab;
pub use run::{print_plain, run};

/// Runtime switches passed from the CLI into the control center.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunOptions {
    pub dry_run: bool,
    pub verbose: bool,
    pub start: Option<Tab>,
}
