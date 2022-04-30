#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::error::Error;
use tokio::sync::mpsc::{channel, Receiver, Sender};

mod clip;
use clip::*;
mod proto;
mod trans;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut clip = Clip::new();
    clip.set_text("UniClip".into()).unwrap();

    let (from_net_tx, mut from_net_rx): (Sender<proto::ClipMsg>, Receiver<proto::ClipMsg>) =
        channel(10);
    let (to_net_tx, to_net_rx): (Sender<proto::ClipMsg>, Receiver<proto::ClipMsg>) = channel(10);

    tokio::spawn(async move { trans::trans(from_net_tx, to_net_rx).await });

    let (clip_tx, clip_rx) = std::sync::mpsc::channel();

    std::thread::spawn(|| {
        let clip_monitor = ClipMonitor::new();
        clip_monitor.notify(clip_tx);
    });

    tokio::spawn(async move {
        loop {
            if let Some(msg) = from_net_rx.recv().await {
                println!("{msg:?}");
                if msg.typ() == proto::clip_msg::MsgType::Text {
                    let text = msg.text();
                    clip.set_text(text.to_owned());
                }
            }
        }
    });

    loop {
        if let Ok(msg) = clip_rx.recv() {
            println!("{msg:?}");

            match msg {
                ClipMsg::Text(text) => {
                    let mut clip_msg = proto::ClipMsg::default();
                    clip_msg.id = 0;
                    clip_msg.set_typ(proto::clip_msg::MsgType::Text);
                    clip_msg.text = Some(text);
                    to_net_tx.send(clip_msg).await;
                }
                ClipMsg::Image(_) => todo!(),
            }
        }
    }
}
