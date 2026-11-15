use std::io::{self, IsTerminal};
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyEventKind},
    execute,
    style::ResetColor,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::tui::actions::{self, Effect, Suspended};
use crate::tui::app::{App, ToastLevel};
use crate::tui::{input, render, views, RunOptions};

type Backend = CrosstermBackend<io::Stdout>;
type AppTerminal = Terminal<Backend>;

/// Restores the terminal on every exit path, including panics.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show, ResetColor);
    }
}

/// Opens the control center, or dumps a plain dashboard when piped.
pub fn run(opts: RunOptions) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        print_plain();
        return Ok(());
    }

    install_panic_hook();
    enable_raw_mode()?;
    let _guard = TerminalGuard;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, Hide)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.clear()?;

    let mut app = App::new();
    if let Some(tab) = opts.start {
        app.set_tab(tab);
    }
    views::ensure_cursor(&mut app);

    event_loop(&mut terminal, &mut app, opts)
}

fn event_loop(terminal: &mut AppTerminal, app: &mut App, opts: RunOptions) -> Result<()> {
    loop {
        app.maybe_refresh();
        if app.toast.as_ref().is_some_and(|toast| !toast.alive()) {
            app.toast = None;
        }
        terminal.draw(|frame| render::draw(frame, app))?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        let effect = input::handle_key(app, key);
        if !apply_effect(terminal, app, effect, opts)? {
            return Ok(());
        }
    }
}

/// Applies chained effects; returns `false` when the app should quit.
fn apply_effect(
    terminal: &mut AppTerminal,
    app: &mut App,
    mut effect: Effect,
    opts: RunOptions,
) -> Result<bool> {
    loop {
        match effect {
            Effect::None => return Ok(true),
            Effect::Quit => return Ok(false),
            Effect::Toast(message, level) => {
                app.toast(message, level);
                return Ok(true);
            }
            Effect::Action(action) => effect = actions::execute(app, action, opts),
            Effect::Suspend(task) => {
                run_suspended(terminal, app, task)?;
                return Ok(true);
            }
        }
    }
}

fn run_suspended(terminal: &mut AppTerminal, app: &mut App, task: Suspended) -> Result<()> {
    let Suspended { label, task, pause } = task;

    terminal.show_cursor()?;
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        Show,
        ResetColor
    )?;

    println!("\n  ▸ {label}\n");
    let result = task(app);
    match &result {
        Ok(message) if !message.is_empty() => println!("  ✔ {message}"),
        Ok(_) => {}
        Err(err) => println!("  ✖ {err:#}"),
    }

    if pause {
        println!("\n  Press Enter to return to ryoiki…");
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer)?;
    }

    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen, Hide)?;
    terminal.clear()?;

    match result {
        Ok(message) => app.toast(message, ToastLevel::Success),
        Err(err) => app.toast(format!("{err:#}"), ToastLevel::Error),
    }
    Ok(())
}

fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show, ResetColor);
        original(info);
    }));
}

/// Plain-text dashboard for non-interactive terminals.
pub fn print_plain() {
    let app = App::new();
    let battery = app
        .snapshot
        .battery_percent
        .map_or_else(|| "n/a".to_string(), |pct| format!("{pct}%"));
    let poweroff = if app.settings.power.poweroff_on_low {
        format!("on at {}%", app.settings.power.poweroff_percent)
    } else {
        "off".to_string()
    };

    println!("\n  領域 Ryoiki · Control Center v{}", app.version);
    println!("  {}", "─".repeat(50));
    println!("  Host            {}", app.snapshot.host);
    println!("  Kernel          {}", app.snapshot.kernel);
    println!("  Uptime          {}", app.snapshot.uptime);
    println!("  CPU             {}", app.snapshot.cpu);
    println!("  Memory          {}", app.snapshot.memory);
    println!("  Disk /          {}", app.snapshot.disk);
    println!(
        "  Battery         {battery}  {}  {}",
        app.snapshot.battery_status,
        app.snapshot.power_label()
    );
    println!("  Charge backend  {}", app.snapshot.charge_backend);
    println!(
        "  Charge ceiling  {}%",
        app.settings.battery.max_charge_percent
    );
    println!("  Auto poweroff   {poweroff}");
    println!(
        "  Watcher         {} ({})",
        app.snapshot.watcher_state,
        if app.snapshot.watcher_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("  Services        {} managed", app.services.len());
    println!(
        "  Settings        {}",
        crate::settings::settings_path().display()
    );
    println!("\n  Run `ryoiki` in an interactive terminal for the full dashboard.");
}
