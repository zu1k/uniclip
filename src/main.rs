#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::mpsc::{channel};

mod proto;
mod clip;
use clip::*;
use prost::Message;

use crate::proto::{clip_msg, ClipMsg};

fn main() {

    let mut msg = ClipMsg::default();
    msg.id = 0;
    msg.set_typ(clip_msg::MsgType::Text);
    msg.text = Some("abcaabbcc".into());
    println!("{msg:?}");
    let msg = msg.encode_to_vec();
    println!("{msg:?}");

    let mut clip = Clip::new();
    clip.set_text("abcfjhdsakhjfas".into()).unwrap();

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
