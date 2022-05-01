#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use config::Config;
use serde::Deserialize;
use std::sync::Arc;

mod clip;
use clip::*;
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
    let (to_net_tx, to_net_rx) = tokio::sync::mpsc::channel(10);

    std::thread::spawn(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(
                async move { uniclip_net::trans(&settings.domain, from_net_tx, to_net_rx).await },
            );
    });

    let clip = Arc::new(Clip::new());
    let monitor_clip = clip.clone();

    std::thread::spawn(move || {
        monitor_clip.notify(|msg| match msg {
            ClipMsg::Text(text) => {
                println!("local clipboard notify text: {text}");
                let mut clip_msg = uniclip_proto::ClipMsg {
                    text: Some(text),
                    ..Default::default()
                };
                clip_msg.set_typ(uniclip_proto::clip_msg::MsgType::Text);
                to_net_tx.blocking_send(clip_msg).unwrap();
            }
            ClipMsg::Image(image) => {
                println!("local clipboard notify image");
                let mut clip_msg = uniclip_proto::ClipMsg {
                    image: Some(uniclip_proto::clip_msg::ImageData {
                        data: image.2,
                        width: image.0 as u32,
                        height: image.1 as u32,
                    }),
                    ..Default::default()
                };
                clip_msg.set_typ(uniclip_proto::clip_msg::MsgType::Image);
                to_net_tx.blocking_send(clip_msg).unwrap();
            }
        });
    });

    std::thread::spawn(move || {
        loop {
            if let Ok(msg) = from_net_rx.recv() {
                match msg.typ() {
                    uniclip_proto::clip_msg::MsgType::Text => {
                        let text = msg.text();
                        println!("receive from net: {text}");
                        clip.clone().set_text(text).unwrap();
                    }
                    uniclip_proto::clip_msg::MsgType::Image => {
                        let image = msg.image.unwrap();
                        println!("receive from net: image");
                        clip.clone()
                            .set_image((image.width as usize, image.height as usize, &image.data))
                            .unwrap();
                    }
                }
            }
        }
    });

    tray::start_tray();
}
