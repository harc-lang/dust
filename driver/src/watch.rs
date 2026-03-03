//! Single-value watch channel for sharing state between threads.
//!
//! The [`Watch`] type holds a single value behind a mutex. The writer
//! overwrites the value whenever it likes, and the reader always gets
//! the most recent value. This is equivalent to a channel with a
//! buffer size of one where new writes overwrite the old value.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::command::DriverMode;

/// A snapshot of the driver's observable state.
///
/// Written by the worker thread after each batch of steps.
/// Read by the GUI thread each frame.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Current driver mode (idle or running).
    pub mode: DriverMode,
    /// Whether simulation state exists.
    pub has_state: bool,
    /// Current iteration count.
    pub iteration: i64,
    /// Current simulation time.
    pub time: f64,
    /// Human-readable status line from the last completed step.
    pub status_text: String,
    /// Named 1D series for line/scatter plots.
    pub linear: HashMap<String, Vec<f64>>,
    /// Named 2D fields for heatmap plots: (rows, cols, row-major data).
    pub planar: HashMap<String, (usize, usize, Vec<f64>)>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            mode: DriverMode::Idle,
            has_state: false,
            iteration: 0,
            time: 0.0,
            status_text: String::new(),
            linear: HashMap::new(),
            planar: HashMap::new(),
        }
    }
}

/// A single-value watch channel.
///
/// Both [`Watch::write`] and [`Watch::read`] acquire the mutex briefly,
/// so contention is negligible as long as neither side holds the lock
/// across expensive work.
#[derive(Clone)]
pub struct Watch<T> {
    inner: Arc<Mutex<T>>,
}

impl<T> Watch<T> {
    /// Create a new watch channel with the given initial value.
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(value)),
        }
    }

    /// Overwrite the current value.
    pub fn write(&self, value: T) {
        *self.inner.lock().unwrap() = value;
    }

    /// Read the current value, cloning it.
    pub fn read(&self) -> T
    where
        T: Clone,
    {
        self.inner.lock().unwrap().clone()
    }

    /// Update the current value in place with a closure.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        f(&mut *self.inner.lock().unwrap());
    }
}
