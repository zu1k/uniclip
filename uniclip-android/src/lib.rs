use std::sync::mpsc::Sender;
use tokio::sync::mpsc::Receiver;
use uniclip_proto::ClipMsg;

struct Net {
    topic: String,
    from_net_tx: Sender<ClipMsg>,
    to_net_rx: Receiver<ClipMsg>,
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
