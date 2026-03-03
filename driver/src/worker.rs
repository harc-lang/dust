//! Worker thread: runs the driver on a background thread, communicating via channels.
//!
//! Commands flow from the frontend to the worker via `cmd_tx`.
//! Events (low-frequency state transitions) flow back via `event_rx`.
//! Continuous data (step progress, plot data) flows through `Watch<Snapshot>`.

use crate::command::{Command, Event};
use crate::driver::Driver;
use crate::solver::Solver;
use crate::watch::{Snapshot, Watch};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

/// Minimum time to spend stepping before checking for commands.
const STEP_BATCH_DURATION: Duration = Duration::from_millis(30);

/// Handle returned by [`spawn`]. Provides command/event channels
/// and a watch channel for reading the latest snapshot.
pub struct DriverHandle {
    pub cmd_tx: Sender<Command>,
    pub event_rx: Receiver<Event>,
    pub snapshot: Watch<Snapshot>,
}

/// Spawn a thread that owns the driver and communicates via channels.
pub fn spawn<S: Solver + Send + 'static>(driver: Driver<S>, init_events: Vec<Event>) -> DriverHandle
where
    S::State: Send,
    S::Physics: Send,
    S::Initial: Send,
    S::Compute: Send,
{
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let snapshot = Watch::new(Snapshot::default());
    let snapshot_writer = snapshot.clone();

    for event in init_events {
        let _ = event_tx.send(event);
    }

    // Write initial snapshot before spawning so the GUI has something to read.
    driver.write_snapshot(&snapshot_writer);

    std::thread::spawn(move || {
        worker_main(driver, cmd_rx, event_tx, snapshot_writer);
    });

    DriverHandle {
        cmd_tx,
        event_rx,
        snapshot,
    }
}

fn worker_main<S: Solver>(
    mut driver: Driver<S>,
    cmd_rx: Receiver<Command>,
    event_tx: Sender<Event>,
    snapshot: Watch<Snapshot>,
) {
    loop {
        if driver.is_running() {
            // Drain all pending commands first
            while let Ok(cmd) = cmd_rx.try_recv() {
                let is_quit = matches!(cmd, Command::Quit);
                for event in driver.accept(cmd) {
                    let _ = event_tx.send(event);
                }
                if is_quit {
                    return;
                }
            }

            // If still running after processing commands, step in a batch
            if driver.is_running() {
                let deadline = Instant::now() + STEP_BATCH_DURATION;
                while driver.is_running() {
                    for event in driver.accept(Command::Run) {
                        let _ = event_tx.send(event);
                    }
                    if Instant::now() >= deadline {
                        break;
                    }
                }
                // Write snapshot once per batch
                driver.write_snapshot(&snapshot);
            } else {
                // Mode changed during stepping (e.g. SimulationDone)
                driver.write_snapshot(&snapshot);
            }
        } else {
            // Idle: block until a command arrives
            match cmd_rx.recv() {
                Ok(cmd) => {
                    let is_quit = matches!(cmd, Command::Quit);
                    for event in driver.accept(cmd) {
                        let _ = event_tx.send(event);
                    }
                    // Write snapshot after every command while idle
                    driver.write_snapshot(&snapshot);
                    if is_quit {
                        return;
                    }
                }
                Err(_) => return,
            }
        }
    }
}
