#![allow(unused_variables)]

#[macro_use]
extern crate lazy_static;

use android_logger::Config;
use jni::{
    objects::{JClass, JObject, JString},
    sys::jstring,
    JNIEnv,
};
use log::{info, trace, Level};
use std::sync::{Arc, Mutex};
use uniclip_proto::{clip_msg::MsgType, ClipMsg};

fn native_activity_create() {
    android_logger::init_once(
        Config::default()
            .with_min_level(Level::Trace)
            .with_tag("uniclip"),
    );
}

lazy_static! {
    static ref TOPIC: Mutex<String> = Mutex::new(String::from("uniclip"));
    static ref TO_NET_TX: Mutex<Option<tokio::sync::mpsc::Sender<ClipMsg>>> = Mutex::new(None);
}

struct Net {
    topic: String,
    from_net_tx: std::sync::mpsc::Sender<ClipMsg>,
    to_net_rx: tokio::sync::mpsc::Receiver<ClipMsg>,
}

impl Net {
    fn start(self) {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                uniclip_net::trans(&self.topic, self.from_net_tx, self.to_net_rx).await
            });
    }
}

pub fn start_net<F>(topic: String, callback: F)
where
    F: Fn(String),
{
    let (from_net_tx, from_net_rx) = std::sync::mpsc::channel();
    let (to_net_tx, to_net_rx) = tokio::sync::mpsc::channel(10);

    {
        TO_NET_TX.lock().unwrap().replace(to_net_tx);
        *TOPIC.lock().unwrap() = topic.clone();
    }

    let net = Net {
        topic,
        from_net_tx,
        to_net_rx,
    };

    let callback = Arc::new(callback);

    std::thread::spawn(move || {
        net.start();
    });

    loop {
        if let Ok(msg) = from_net_rx.recv() {
            match msg.typ() {
                uniclip_proto::clip_msg::MsgType::Text => {
                    let text = msg.text();
                    info!("receive from net: {text}");
                    callback.clone()(text.to_string());
                }
                uniclip_proto::clip_msg::MsgType::Image => {
                    let image = msg.image.unwrap();
                    info!("receive from net: image");
                }
            }
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_zu1k_uniclip_MainActivity_start(
    env: JNIEnv,
    _class: JClass,
    topic: JString,
    callback: JObject,
) {
    native_activity_create();

    let topic: String = env
        .get_string(topic)
        .expect("Couldn't get java string!")
        .into();

    trace!("topic: {topic}");

    let on_net_reveive = |text: String| {
        let output = env.new_string(text).expect("Couldn't create java string!");
        env.call_method(
            callback,
            "copyToClipboard",
            "(Ljava/lang/String;)V",
            &[output.into()],
        )
        .unwrap();
    };

    start_net(topic, on_net_reveive)
}

#[no_mangle]
pub extern "system" fn Java_com_zu1k_uniclip_ClipboardMonitorService_clipPublishText(
    env: JNIEnv,
    _class: JClass,
    text: JString,
) {
    let text: String = env
        .get_string(text)
        .expect("Couldn't get java string!")
        .into();

    let mut msg = ClipMsg::default();
    msg.set_typ(MsgType::Text);
    msg.text = Some(text);

    TO_NET_TX
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .blocking_send(msg)
        .unwrap();
}

#[no_mangle]
pub extern "system" fn Java_com_zu1k_uniclip_MainActivity_stringFromJNI(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let output = env
        .new_string("uniclip from Rust lib".to_string())
        .expect("Couldn't create java string!");
    output.into_inner()
}
