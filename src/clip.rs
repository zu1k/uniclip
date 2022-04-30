use arboard::*;
use std::{
    sync::{Arc, RwLock},
    thread::sleep,
    time::Duration,
};

#[derive(Debug)]
pub enum ClipMsg {
    Text(String),
    Image((usize, usize, Vec<u8>)),
}

impl ClipMsg {
    pub fn text(txt: String) -> ClipMsg {
        ClipMsg::Text(txt)
    }

    pub fn image(width: usize, height: usize, data: &[u8]) -> ClipMsg {
        ClipMsg::Image((width, height, data.to_owned()))
    }
}

pub struct Clip {
    text: RwLock<String>,
    image_info: RwLock<(usize, usize, usize)>, // width, height, len

    delay_millis: u64,
}

impl Clip {
    pub fn new() -> Self {
        Self {
            text: RwLock::new("".into()),
            image_info: RwLock::new((0, 0, 0)),

            delay_millis: 200,
        }
    }

    pub fn notify<F>(self: Arc<Self>, on_clipboard_change: F)
    where
        F: Fn(ClipMsg),
    {
        let mut clip = Clipboard::new().unwrap();
        loop {
            if let Ok(text) = clip.get_text() {
                let origin = { self.text.read().unwrap().to_owned() };
                if text != origin {
                    {
                        *self.text.write().unwrap() = text.clone();
                    }
                    on_clipboard_change(ClipMsg::text(text));
                }
            }

            if let Ok(image) = clip.get_image() {
                let image_info = (image.width, image.height, image.bytes.len());
                let origin = { self.image_info.read().unwrap().to_owned() };
                if image_info != origin {
                    {
                        *self.image_info.write().unwrap() = image_info;
                    }
                    on_clipboard_change(ClipMsg::image(image.width, image.height, &image.bytes));
                }
            }

            sleep(Duration::from_millis(self.delay_millis));
        }
    }

    pub fn set_text(self: Arc<Self>, text: &str) -> anyhow::Result<()> {
        let mut clip = Clipboard::new().unwrap();
        clip.set_text(text.to_owned())?;
        {
            *self.text.write().unwrap() = text.to_owned();
        }
        sleep(Duration::from_secs(1));
        Ok(())
    }

    pub fn set_image(self: Arc<Self>, image: (usize, usize, &[u8])) -> anyhow::Result<()> {
        let mut clip = Clipboard::new().unwrap();
        clip.set_image(ImageData {
            width: image.0,
            height: image.1,
            bytes: image.2.into(),
        })?;
        {
            *self.image_info.write().unwrap() = (image.0, image.1, image.2.len());
        }
        sleep(Duration::from_secs(1));
        Ok(())
    }
}
