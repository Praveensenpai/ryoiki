use std::process::Command;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::tui::app::{Action, App, ToastLevel};
use crate::tui::RunOptions;

/// A long or privileged task that must run outside the alternate screen.
pub type Task = Box<dyn FnOnce(&mut App) -> Result<String>>;

const INSTALL_HOOKS_ARGS: [&str; 2] = ["notify", "install-hooks"];

pub struct Suspended {
    pub label: String,
    pub task: Task,
    pub pause: bool,
}

/// What the event loop should do after a key press.
pub enum Effect {
    None,
    Quit,
    Toast(String, ToastLevel),
    Action(Action),
    Suspend(Suspended),
}

/// Executes one semantic action.
pub fn execute(app: &mut App, action: Action, opts: RunOptions) -> Effect {
    match action {
        Action::None => Effect::None,
        Action::Refresh => refresh(app),
        Action::SaveSettings => {
            app.save_settings();
            Effect::None
        }
        Action::ReloadSettings => {
            app.reload_settings();
            Effect::None
        }
        Action::SetPoweroffEnabled(enabled) => set_poweroff(app, enabled),
        Action::ApplyChargeLimit => apply_charge_limit(app),
        Action::ResetAlerts => reset_alerts(app),
        Action::RestartWatcher => watcher("restart", opts),
        Action::EnableWatcher => watcher("enable", opts),
        Action::DisableWatcher => watcher("disable", opts),
        Action::TestNotification => test_notification(app),
        Action::InstallHooks => install_hooks(),
        Action::CheckUpdate => check_update(),
        Action::SelfUpdate => self_update(app),
        Action::RunProvision => run_provision(app, opts),
        Action::RestartService { name, is_user } => restart_service(name, is_user, opts),
        Action::RestartContainer(name) => restart_container(name),
        Action::LaunchMedia => media_manager(),
        Action::PruneDryRun => prune_preview(app),
        Action::ApplyPrunePolicy => apply_prune_policy(app),
        Action::DisablePruneTimer => disable_prune_timer(),
        Action::ShowStorage => storage_report(),
        Action::Edit(target) => {
            app.open_input(target);
            Effect::None
        }
        Action::ClearResume => clear_resume(),
    }
}

fn refresh(app: &mut App) -> Effect {
    app.refresh();
    Effect::Toast("snapshot refreshed".to_string(), ToastLevel::Info)
}

fn set_poweroff(app: &mut App, enabled: bool) -> Effect {
    app.settings.power.poweroff_on_low = enabled;
    app.save_settings();
    let state = if enabled { "enabled" } else { "disabled" };
    Effect::Toast(format!("auto poweroff {state}"), ToastLevel::Success)
}

fn apply_charge_limit(app: &mut App) -> Effect {
    let limit = app.settings.battery.max_charge_percent;
    suspend(
        format!("Applying a {limit}% charge ceiling"),
        false,
        move |app| {
            let message = crate::charge_limit::apply_limit_programmatic(limit)?;
            app.refresh();
            Ok(message)
        },
    )
}

fn reset_alerts(app: &mut App) -> Effect {
    app.settings.battery.alert_thresholds = crate::settings::DEFAULT_ALERTS.to_vec();
    app.save_settings();
    Effect::Toast("alert thresholds reset".to_string(), ToastLevel::Success)
}

fn install_hooks() -> Effect {
    suspend("Installing notification hooks", true, move |app| {
        let exe = std::env::current_exe().context("failed to locate the ryoiki binary")?;
        ensure_sudo()?;
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let status = Command::new("sudo")
            .arg("env")
            .arg(format!("HOME={home}"))
            .arg(exe)
            .args(INSTALL_HOOKS_ARGS)
            .status()
            .context("failed to run the hook installer")?;
        if !status.success() {
            bail!("hook installation failed");
        }
        app.refresh();
        Ok("notification hooks installed".to_string())
    })
}

fn check_update() -> Effect {
    suspend("Checking GitHub releases", false, move |app| {
        let latest = with_timeout(10, crate::updater::latest_version)?;
        let message = if latest == app.version {
            format!("up to date (v{})", app.version)
        } else {
            format!("v{latest} available")
        };
        app.latest_version = Some(latest);
        app.update_status.clone_from(&message);
        Ok(message)
    })
}

fn self_update(app: &mut App) -> Effect {
    let version = app.version;
    suspend("Installing the latest release", true, move |app| {
        crate::updater::run_self_update(version)?;
        app.update_status = "updated — restart ryoiki".to_string();
        Ok("update installed — restart ryoiki".to_string())
    })
}

fn clear_resume() -> Effect {
    crate::state::RunState::clear();
    Effect::Toast("resume state cleared".to_string(), ToastLevel::Success)
}

fn suspend<F>(label: impl Into<String>, pause: bool, task: F) -> Effect
where
    F: FnOnce(&mut App) -> Result<String> + 'static,
{
    Effect::Suspend(Suspended {
        label: label.into(),
        task: Box::new(task),
        pause,
    })
}

fn restart_service(name: String, is_user: bool, opts: RunOptions) -> Effect {
    suspend(format!("Restarting {name}"), false, move |app| {
        if is_user {
            crate::bot::services::restart_managed_service(&name)?;
        } else {
            run_root("systemctl", &["restart", &name], opts)?;
        }
        app.refresh();
        Ok(format!("{name} restarted"))
    })
}

fn restart_container(name: String) -> Effect {
    suspend(format!("Restarting container {name}"), false, move |app| {
        crate::bot::services::restart_docker_container(&name)?;
        app.refresh();
        Ok(format!("container {name} restarted"))
    })
}

fn media_manager() -> Effect {
    suspend("Opening the media manager", true, move |app| {
        crate::modules::media::handle_media_cli(None)?;
        app.refresh();
        Ok("media manager closed".to_string())
    })
}

fn prune_preview(app: &App) -> Effect {
    let threshold = app.settings.storage.prune_threshold;
    let target = app.settings.storage.prune_target;
    suspend("Previewing media prune", true, move |app| {
        let options = crate::modules::media::pruner::PruneOptions {
            threshold_pct: threshold,
            target_pct: target,
            dry_run: true,
        };
        crate::modules::media::pruner::run_prune(options)?;
        app.refresh();
        Ok("prune preview finished".to_string())
    })
}

fn apply_prune_policy(app: &App) -> Effect {
    let threshold = app.settings.storage.prune_threshold;
    let target = app.settings.storage.prune_target;
    suspend(
        format!("Applying the {threshold}% → {target}% auto-prune policy"),
        true,
        move |app| {
            crate::settings::validate_prune(threshold, target)?;
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            crate::modules::media::prune_timer::deploy_prune_timer(&home, threshold, target)?;
            app.refresh();
            Ok(format!(
                "auto-prune timer active at {threshold}% → {target}%"
            ))
        },
    )
}

fn disable_prune_timer() -> Effect {
    suspend("Disabling the auto-prune timer", true, move |app| {
        crate::modules::media::prune_timer::disable_prune_timer()?;
        app.refresh();
        Ok("auto-prune timer disabled".to_string())
    })
}

fn storage_report() -> Effect {
    suspend("Collecting the storage report", true, move |app| {
        crate::modules::media::status::show_storage_status();
        app.refresh();
        Ok("storage report finished".to_string())
    })
}

fn watcher(action: &str, opts: RunOptions) -> Effect {
    let unit = "ryoiki-battery-watch.service";
    let action = action.to_string();
    let label = format!("Changing the battery watcher ({action})");
    let past = match action.as_str() {
        "enable" => "enabled",
        "disable" => "disabled",
        _ => "restarted",
    };
    suspend(label, false, move |app| {
        let args: Vec<&str> = match action.as_str() {
            "enable" => vec!["enable", "--now", unit],
            "disable" => vec!["disable", "--now", unit],
            _ => vec!["restart", unit],
        };
        run_root("systemctl", &args, opts)?;
        app.refresh();
        Ok(format!("battery watcher {past}"))
    })
}

fn test_notification(app: &App) -> Effect {
    let Some(config) = app.telegram.clone() else {
        return Effect::Toast("telegram is not configured".to_string(), ToastLevel::Warn);
    };
    if config.bot_token.is_empty() || config.chat_id.is_empty() {
        return Effect::Toast(
            "bot token and chat id are required".to_string(),
            ToastLevel::Warn,
        );
    }
    let host = app.snapshot.host.clone();
    suspend("Sending a test notification", false, move |app| {
        let fields = [("🖥 Host:", host.as_str()), ("⚙ Source:", "control center")];
        let card = crate::notify::client::format_card(
            "Control Center",
            "✅ <b>RYOIKI IS ONLINE</b>",
            &fields,
        );
        let token = config.bot_token.clone();
        let chat = config.chat_id.clone();
        with_timeout(12, move || {
            crate::notify::client::send_alert(&token, &chat, &card)
        })?;
        app.refresh();
        Ok("test notification sent".to_string())
    })
}

fn run_provision(app: &App, opts: RunOptions) -> Effect {
    let selected: Vec<String> = app
        .provision
        .iter()
        .filter(|module| module.selected)
        .map(|module| module.id.clone())
        .collect();
    if selected.is_empty() {
        return Effect::Toast("select at least one module".to_string(), ToastLevel::Warn);
    }
    let total = crate::modules::resolve_dependencies(&selected).len();
    suspend(
        format!("Provisioning {total} module(s)"),
        true,
        move |app| provision(app, &selected, opts),
    )
}

fn provision(app: &mut App, selected: &[String], opts: RunOptions) -> Result<String> {
    let resolved = crate::modules::resolve_dependencies(selected);
    let mut runner = crate::runner::Runner::new(opts.dry_run, opts.verbose)?;
    if crate::modules::requires_sudo(&resolved) {
        runner.ensure_sudo()?;
    }

    let catalog = crate::modules::get_available_modules();
    let total = resolved.len();
    for (index, id) in resolved.iter().enumerate() {
        if let Some(meta) = catalog.iter().find(|module| module.id == id.as_str()) {
            println!("  [{}/{}] {}", index + 1, total, meta.title);
        }
        crate::modules::execute_module(id, &mut runner, true)?;
        if let Some(row) = app.provision.iter_mut().find(|row| row.id == *id) {
            row.installed = true;
        }
    }
    app.refresh();
    Ok(format!("provisioned {total} module(s)"))
}

/// Runs a blocking task on a worker thread with a hard wall-clock limit.
///
/// DNS resolution can block past the HTTP client timeout, so the UI must never
/// wait on it forever. The worker is detached; a late result is discarded.
fn with_timeout<T, F>(secs: u64, task: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(task());
    });
    match rx.recv_timeout(Duration::from_secs(secs)) {
        Ok(result) => result,
        Err(_) => bail!("request timed out after {secs}s"),
    }
}

fn ensure_sudo() -> Result<()> {
    if unsafe { libc::geteuid() } == 0 {
        return Ok(());
    }
    let cached = Command::new("sudo")
        .args(["-n", "true"])
        .status()
        .is_ok_and(|status| status.success());
    if cached {
        return Ok(());
    }
    println!("  sudo is required; enter your password if prompted.");
    let status = Command::new("sudo")
        .arg("-v")
        .status()
        .context("failed to run sudo")?;
    if !status.success() {
        bail!("sudo authentication failed");
    }
    Ok(())
}

fn run_root(program: &str, args: &[&str], opts: RunOptions) -> Result<()> {
    if opts.dry_run {
        println!("  dry-run: {program} {}", args.join(" "));
        return Ok(());
    }
    let is_root = unsafe { libc::geteuid() } == 0;
    if !is_root {
        ensure_sudo()?;
    }
    let output = if is_root {
        Command::new(program).args(args).output()
    } else {
        Command::new("sudo")
            .args(["-n", program])
            .args(args)
            .output()
    }
    .with_context(|| format!("failed to run {program}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{program} {} failed: {}", args.join(" "), stderr.trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::INSTALL_HOOKS_ARGS;

    #[test]
    fn install_hooks_uses_the_real_cli_subcommand() {
        assert_eq!(INSTALL_HOOKS_ARGS, ["notify", "install-hooks"]);
    }
}
