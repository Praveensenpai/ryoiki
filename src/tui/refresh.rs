use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};

use crate::tui::system::{self, CpuSampler, ServiceRow, Snapshot};

/// Everything the worker re-probes for the interface.
pub struct Payload {
    pub snapshot: Snapshot,
    pub services: Vec<ServiceRow>,
    pub installed: Vec<(String, bool)>,
}

/// Result of a non-blocking poll of the worker.
pub enum Poll {
    Ready(Box<Payload>),
    Idle,
    Stopped,
}

/// Owns the background thread that probes the host.
///
/// The UI thread only sends requests and drains results; it never waits on a
/// probe, so key handling and redraws stay responsive even when ACPI or Docker
/// is slow.
pub struct Worker {
    requests: Sender<()>,
    results: Receiver<Payload>,
    _handle: JoinHandle<()>,
}

impl Worker {
    pub fn spawn(module_ids: Vec<String>) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<()>();
        let (result_tx, result_rx) = mpsc::channel::<Payload>();

        let handle = thread::spawn(move || {
            let mut sampler = CpuSampler::new();
            let _ = sampler.sample();
            while request_rx.recv().is_ok() {
                let payload = collect(&module_ids, &mut sampler);
                if result_tx.send(payload).is_err() {
                    break;
                }
            }
        });

        Self {
            requests: request_tx,
            results: result_rx,
            _handle: handle,
        }
    }

    /// Queues a refresh. Returns `false` if the worker has stopped.
    pub fn request(&self) -> bool {
        self.requests.send(()).is_ok()
    }

    /// Drains one completed refresh without blocking.
    pub fn poll(&self) -> Poll {
        match self.results.try_recv() {
            Ok(payload) => Poll::Ready(Box::new(payload)),
            Err(TryRecvError::Empty) => Poll::Idle,
            Err(TryRecvError::Disconnected) => Poll::Stopped,
        }
    }
}

fn collect(module_ids: &[String], sampler: &mut CpuSampler) -> Payload {
    let snapshot = system::collect_for_worker(sampler);
    let services = system::collect_services();
    let installed = module_ids
        .iter()
        .map(|id| (id.clone(), system::module_installed(id)))
        .collect();

    Payload {
        snapshot,
        services,
        installed,
    }
}
