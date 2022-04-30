#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use config::Config;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::mpsc::{channel, Receiver, Sender};

mod clip;
use clip::*;
mod proto;
mod trans;
mod tray;

#[derive(Debug, Default, Deserialize)]
pub struct Settings {
    domain: String,
}

fn main() {
    let settings = Config::builder()
        .add_source(config::File::with_name("settings"))
        .add_source(config::Environment::with_prefix("UNICLIP"))
        .build()
        .unwrap();

    let settings = settings.try_deserialize::<Settings>().unwrap();

    let (from_net_tx, from_net_rx) = std::sync::mpsc::channel();
    let (to_net_tx, to_net_rx): (Sender<proto::ClipMsg>, Receiver<proto::ClipMsg>) = channel(10);

    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move { trans::trans(&settings, from_net_tx, to_net_rx).await });
    });

    let clip = Arc::new(Clip::new());
    let monitor_clip = clip.clone();

    std::thread::spawn(move || {
        monitor_clip.notify(|msg| {
            println!("local clipboard notify: {msg:?}");

            match msg {
                ClipMsg::Text(text) => {
                    let mut clip_msg = proto::ClipMsg {
                        text: Some(text),
                        ..Default::default()
                    };
                    clip_msg.set_typ(proto::clip_msg::MsgType::Text);
                    to_net_tx.blocking_send(clip_msg).unwrap();
                }
                ClipMsg::Image(_) => todo!(),
            }
        });
    });

    std::thread::spawn(move || {
        loop {
            if let Ok(msg) = from_net_rx.recv() {
                if msg.typ() == proto::clip_msg::MsgType::Text {
                    let text = msg.text();
                    println!("receive from net: {text}");
                    clip.clone().set_text(text).unwrap();
                }
            }
        }
    });

    tray::start_tray();
}
