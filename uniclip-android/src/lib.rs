use jni::{
    objects::{GlobalRef, JClass, JObject, JString},
    sys::{jbyteArray, jint, jlong, jstring},
    JNIEnv,
};
use uniclip_proto::ClipMsg;

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

pub fn start_net(topic: String) {
    let (from_net_tx, from_net_rx) = std::sync::mpsc::channel();
    let (to_net_tx, to_net_rx) = tokio::sync::mpsc::channel(10);

    let net = Net {
        topic,
        from_net_tx,
        to_net_rx,
    };

    net.start();
}

#[no_mangle]
pub extern "system" fn Java_HelloWorld_hello(env: JNIEnv, _class: JClass, topic: JString) {
    let topic: String = env
        .get_string(topic)
        .expect("Couldn't get java string!")
        .into();

    start_net(topic)
}
