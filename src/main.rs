#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::mpsc::{channel};

mod clip;
use clip::*;

fn main() {
    let _clip = Clip::new();

    let (tx, rx) = channel();

    std::thread::spawn(|| {
        let clip_monitor = ClipMonitor::new();
        clip_monitor.notify(tx);
    });

    loop {
        let msg = rx.recv().unwrap();
        println!("{msg:?}");
    }
}
