// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

type CheckpointObserver = dyn Fn(&'static str) + Send + Sync;

struct CancellationInner {
    cancelled: AtomicBool,
    observer: Option<Arc<CheckpointObserver>>,
    reported: Mutex<std::collections::HashSet<&'static str>>,
}

/// Cooperative cancellation shared by NDJSON requests and long-running core work.
///
/// Clones observe the same flag, so the sidecar's stdin thread can cancel work
/// while a request worker is scanning entries, dictionaries, or waiting for an
/// external MT process.
#[derive(Clone)]
pub struct CancellationToken {
    inner: Arc<CancellationInner>,
}

impl std::fmt::Debug for CancellationToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CancellationToken")
            .field("cancelled", &self.is_cancelled())
            .finish_non_exhaustive()
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self {
            inner: Arc::new(CancellationInner {
                cancelled: AtomicBool::new(false),
                observer: None,
                reported: Mutex::new(std::collections::HashSet::new()),
            }),
        }
    }
}

impl CancellationToken {
    /// Construct a request token that reports the first visit to each named
    /// product checkpoint. The sidecar uses this for optional JSON-RPC progress
    /// notifications; ordinary callers retain the zero-observer fast path.
    pub fn with_checkpoint_observer(
        observer: impl Fn(&'static str) + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner: Arc::new(CancellationInner {
                cancelled: AtomicBool::new(false),
                observer: Some(Arc::new(observer)),
                reported: Mutex::new(std::collections::HashSet::new()),
            }),
        }
    }

    pub fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    /// Report a stable operation checkpoint and return the current cancel state.
    ///
    /// Checkpoint names are de-duplicated per request so loops can expose useful
    /// progress without flooding the NDJSON channel.
    pub fn checkpoint(&self, name: &'static str) -> bool {
        if let Some(observer) = &self.inner.observer {
            let first = self
                .inner
                .reported
                .lock()
                .map(|mut reported| reported.insert(name))
                .unwrap_or(false);
            if first {
                observer(name);
            }
        }
        self.is_cancelled()
    }
}
