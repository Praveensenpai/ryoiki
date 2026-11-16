use std::time::{Duration, Instant};

use crate::tui::refresh::{Payload, Poll};

use super::{App, RefreshState, ToastLevel};

impl App {
    /// Queues a refresh on the background worker.
    ///
    /// Returns `false` when a refresh is already running (a follow-up is queued)
    /// or when the worker has stopped.
    pub fn request_refresh(&mut self) -> bool {
        match self.refresh_state {
            RefreshState::Stopped => {
                self.toast("background refresh unavailable", ToastLevel::Error);
                return false;
            }
            RefreshState::Running => {
                self.refresh_state = RefreshState::RunningAgain;
                return false;
            }
            RefreshState::RunningAgain => return false,
            RefreshState::Idle => {}
        }
        if self.worker.request() {
            self.refresh_state = RefreshState::Running;
            true
        } else {
            self.refresh_state = RefreshState::Stopped;
            self.toast("background refresh stopped", ToastLevel::Error);
            false
        }
    }

    /// Applies a completed refresh and starts a queued follow-up if needed.
    pub fn poll_refresh(&mut self) {
        match self.worker.poll() {
            Poll::Ready(payload) => {
                self.apply_payload(*payload);
                self.finish_refresh();
            }
            Poll::Idle => {}
            Poll::Stopped => self.stop_refresh(),
        }
    }

    /// Queues a refresh once the configured interval elapses.
    pub fn maybe_request_refresh(&mut self) {
        let interval = Duration::from_secs(u64::from(self.settings.ui.refresh_secs.max(1)));
        if self.refresh_state == RefreshState::Idle && self.last_refresh.elapsed() >= interval {
            let _ = self.request_refresh();
        }
    }

    /// Whether a background refresh is currently running.
    #[must_use]
    pub fn is_refreshing(&self) -> bool {
        matches!(
            self.refresh_state,
            RefreshState::Running | RefreshState::RunningAgain
        )
    }

    fn apply_payload(&mut self, payload: Payload) {
        self.snapshot = payload.snapshot;
        self.services = payload.services;
        for (id, installed) in payload.installed {
            if let Some(row) = self.provision.iter_mut().find(|row| row.id == id) {
                row.installed = installed;
            }
        }
    }

    fn finish_refresh(&mut self) {
        let refresh_again = self.refresh_state == RefreshState::RunningAgain;
        self.refresh_state = RefreshState::Idle;
        self.last_refresh = Instant::now();
        if refresh_again {
            let _ = self.request_refresh();
        }
    }

    fn stop_refresh(&mut self) {
        if self.refresh_state == RefreshState::Stopped {
            return;
        }
        self.refresh_state = RefreshState::Stopped;
        self.toast("background refresh stopped", ToastLevel::Error);
    }
}
