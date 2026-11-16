use std::time::{Duration, Instant};

use crate::notify::config::TelegramConfig;
use crate::settings::Settings;
use crate::tui::refresh::Worker;
use crate::tui::system::{self, ServiceRow, Snapshot};

mod forms;
mod refresh;
pub use forms::mask_secret;
use forms::{provision_rows, settings_warning, telegram_state};

/// Top-level navigation destinations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Provision,
    Battery,
    Notifications,
    Services,
    Media,
    Updates,
    Settings,
}

const TAB_COUNT: usize = 8;

impl Tab {
    pub const ALL: [Self; TAB_COUNT] = [
        Self::Dashboard,
        Self::Provision,
        Self::Battery,
        Self::Notifications,
        Self::Services,
        Self::Media,
        Self::Updates,
        Self::Settings,
    ];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Provision => "Provision",
            Self::Battery => "Battery",
            Self::Notifications => "Notifications",
            Self::Services => "Services",
            Self::Media => "Media",
            Self::Updates => "Updates",
            Self::Settings => "Settings",
        }
    }

    #[must_use]
    pub const fn subtitle(self) -> &'static str {
        match self {
            Self::Dashboard => "Live system overview",
            Self::Provision => "Install and refresh server modules",
            Self::Battery => "Charge ceiling and low-battery policy",
            Self::Notifications => "Telegram cards and login hooks",
            Self::Services => "systemd units and Docker containers",
            Self::Media => "Organize, prune and inspect storage",
            Self::Updates => "Keep the ryoiki binary current",
            Self::Settings => "Paths, refresh rate and local state",
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Dashboard => 0,
            Self::Provision => 1,
            Self::Battery => 2,
            Self::Notifications => 3,
            Self::Services => 4,
            Self::Media => 5,
            Self::Updates => 6,
            Self::Settings => 7,
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % TAB_COUNT]
    }

    #[must_use]
    pub const fn prev(self) -> Self {
        Self::ALL[(self.index() + TAB_COUNT - 1) % TAB_COUNT]
    }
}

/// Which pane currently owns the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Nav,
    Content,
}

/// Lifecycle of the background refresh worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RefreshState {
    Idle,
    Running,
    RunningAgain,
    Stopped,
}

/// Editable text fields opened as a modal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputTarget {
    BotToken,
    ChatId,
    ServerName,
    AlertThresholds,
    Timezone,
}

/// Every state transition the interface can request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Refresh,
    SaveSettings,
    ReloadSettings,
    SetPoweroffEnabled(bool),
    ApplyChargeLimit,
    ResetAlerts,
    RestartWatcher,
    EnableWatcher,
    DisableWatcher,
    TestNotification,
    InstallHooks,
    CheckUpdate,
    SelfUpdate,
    RunProvision,
    RestartService { name: String, is_user: bool },
    RestartContainer(String),
    LaunchMedia,
    PruneDryRun,
    ApplyPrunePolicy,
    DisablePruneTimer,
    ShowStorage,
    Edit(InputTarget),
    ClearResume,
}

/// Severity of the transient footer message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warn,
    Error,
}

/// Transient footer message with a short lifetime.
#[derive(Clone, Debug)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub at: Instant,
}

impl Toast {
    #[must_use]
    pub fn alive(&self) -> bool {
        self.at.elapsed() < Duration::from_secs(6)
    }
}

/// A destructive or privileged action awaiting confirmation.
#[derive(Clone, Debug)]
pub struct Confirm {
    pub title: String,
    pub body: String,
    pub action: Action,
}

/// Single-line text editor opened over the dashboard.
#[derive(Clone, Debug)]
pub struct TextInput {
    pub target: InputTarget,
    pub label: String,
    pub value: String,
}

/// A provisioning module as shown in the checklist.
#[derive(Clone, Debug)]
pub struct ProvisionRow {
    pub id: String,
    pub title: String,
    pub description: String,
    pub selected: bool,
    pub installed: bool,
}

/// Root application state.
pub struct App {
    pub tab: Tab,
    pub focus: Focus,
    pub cursors: [usize; TAB_COUNT],
    pub settings: Settings,
    pub settings_warning: Option<String>,
    pub telegram: Option<TelegramConfig>,
    pub telegram_error: Option<String>,
    pub snapshot: Snapshot,
    pub provision: Vec<ProvisionRow>,
    pub services: Vec<ServiceRow>,
    pub input: Option<TextInput>,
    pub confirm: Option<Confirm>,
    pub help: bool,
    pub toast: Option<Toast>,
    pub latest_version: Option<String>,
    pub update_status: String,
    pub version: &'static str,
    worker: Worker,
    refresh_state: RefreshState,
    last_refresh: Instant,
}

impl App {
    /// Boots the interface and starts an asynchronous host refresh.
    #[must_use]
    pub fn new() -> Self {
        let provision = provision_rows();
        let module_ids = provision.iter().map(|row| row.id.clone()).collect();
        let snapshot = system::collect_fast();
        let mut app = Self::boot(provision, Worker::spawn(module_ids), snapshot);
        app.request_refresh();
        app
    }

    /// Boots the interface and probes the host synchronously for plain output.
    #[must_use]
    pub fn new_plain() -> Self {
        let provision = provision_rows();
        let module_ids = provision.iter().map(|row| row.id.clone()).collect();
        let snapshot = system::collect();
        let mut app = Self::boot(provision, Worker::spawn(module_ids), snapshot);
        app.services = system::collect_services();
        app
    }

    fn boot(provision: Vec<ProvisionRow>, worker: Worker, snapshot: Snapshot) -> Self {
        let (telegram, telegram_error) = telegram_state();
        Self {
            tab: Tab::Dashboard,
            focus: Focus::Content,
            cursors: [0; TAB_COUNT],
            settings: crate::settings::load_or_default(),
            settings_warning: settings_warning(),
            telegram,
            telegram_error,
            snapshot,
            provision,
            services: Vec::new(),
            input: None,
            confirm: None,
            help: false,
            toast: None,
            latest_version: None,
            update_status: "not checked".to_string(),
            version: env!("CARGO_PKG_VERSION"),
            worker,
            refresh_state: RefreshState::Idle,
            last_refresh: Instant::now(),
        }
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursors[self.tab.index()]
    }

    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursors[self.tab.index()] = cursor;
    }

    pub fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.focus = Focus::Content;
    }

    pub fn next_tab(&mut self) {
        self.set_tab(self.tab.next());
    }

    pub fn prev_tab(&mut self) {
        self.set_tab(self.tab.prev());
    }

    pub fn toast(&mut self, message: impl Into<String>, level: ToastLevel) {
        self.toast = Some(Toast {
            message: message.into(),
            level,
            at: Instant::now(),
        });
    }

    /// Validates and persists the current settings.
    pub fn save_settings(&mut self) {
        match crate::settings::save(&self.settings) {
            Ok(_) => self.toast("settings saved", ToastLevel::Success),
            Err(err) => self.toast(format!("save failed: {err}"), ToastLevel::Error),
        }
    }

    /// Re-reads settings and the Telegram config from disk.
    pub fn reload_settings(&mut self) {
        self.settings = crate::settings::load_or_default();
        self.settings_warning = settings_warning();
        let (telegram, telegram_error) = telegram_state();
        self.telegram = telegram;
        self.telegram_error = telegram_error;
        self.toast("settings reloaded", ToastLevel::Success);
    }

    #[must_use]
    pub fn telegram_ready(&self) -> bool {
        self.telegram
            .as_ref()
            .is_some_and(|cfg| !cfg.bot_token.is_empty() && !cfg.chat_id.is_empty())
    }

    #[must_use]
    pub fn telegram_status(&self) -> &'static str {
        if self.telegram_ready() {
            "configured"
        } else if self.telegram.is_some() {
            "incomplete"
        } else {
            "not configured"
        }
    }
}
