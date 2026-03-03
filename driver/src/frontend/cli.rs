//! Fire-and-forget CLI frontend.
//!
//! Sends `Run`, prints events to stdout, exits when simulation completes.

use crate::command::{Command, Event};
use crate::worker::DriverHandle;

pub fn run(handle: DriverHandle) {
    handle.cmd_tx.send(Command::Run).unwrap();

    // Print status from snapshot periodically while running
    let snapshot = handle.snapshot.clone();
    let printer = std::thread::spawn(move || {
        let mut last_iteration = -1;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let snap = snapshot.read();
            if snap.iteration != last_iteration && !snap.status_text.is_empty() {
                println!("{}", snap.status_text);
                last_iteration = snap.iteration;
            }
            if !snap.has_state || snap.mode == crate::command::DriverMode::Idle {
                break;
            }
        }
    });

    loop {
        match handle.event_rx.recv() {
            Ok(Event::CheckpointWritten { path }) => println!("wrote {}", path),
            Ok(Event::SimulationDone) => {
                // Print final snapshot
                let snap = handle.snapshot.read();
                if !snap.status_text.is_empty() {
                    println!("{}", snap.status_text);
                }
                let _ = handle.cmd_tx.send(Command::Quit);
            }
            Ok(Event::Error(e)) => eprintln!("error: {}", e),
            Ok(Event::Finished) => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }

    printer.join().unwrap();
    println!();
}
