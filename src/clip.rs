use arboard::*;
use std::{
    sync::mpsc::Sender,
    thread::sleep,
    time::Duration,
};

#[derive(Debug)]
pub enum ClipMsg {
    Text(String),
    Image(Vec<u8>),
}

impl ClipMsg {
    pub fn text(txt: String) -> ClipMsg {
        ClipMsg::Text(txt)
    }

    pub fn image(data: &[u8]) -> ClipMsg {
        ClipMsg::Image(data.to_owned())
    }
}

pub struct Clip {
    clip: Clipboard,
}

impl Clip {
    pub fn new() -> Self {
        let clipboard = Clipboard::new().unwrap();

        Self { clip: clipboard }
    }

    pub fn set_text(mut self, text: String) -> anyhow::Result<()> {
        self.clip.set_text(text)?;
        Ok(())
    }
}

pub struct ClipMonitor {
    clip: Clipboard,

    text: String,
    image_info: (usize, usize, usize), // width, height, len

    delay_millis: u64,
}

impl ClipMonitor {
    pub fn new() -> Self {
        let clipboard = Clipboard::new().unwrap();

        Self {
            clip: clipboard,
            text: "".into(),
            image_info: (0, 0, 0),

            delay_millis: 200,
        }
    }

    pub fn notify(mut self, tx: Sender<ClipMsg>) {
        loop {
            if let Ok(text) = self.clip.get_text() {
                if text != self.text {
                    // new text, set and trans
                    self.text = text.clone();
                    tx.send(ClipMsg::text(text)).unwrap();
                }
            }

            if let Ok(image) = self.clip.get_image() {
                let image_info = (image.width, image.height, image.bytes.len());
                if  image_info != self.image_info {
                    self.image_info = image_info;
                    tx.send(ClipMsg::image(&image.bytes)).unwrap();
                }
            }

            sleep(Duration::from_millis(self.delay_millis));
        }
    }
}
