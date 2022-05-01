use tray_item::TrayItem;

cfg_if::cfg_if! {
    if #[cfg(windows)] {
        use std::{process, sync::mpsc};
        enum Message {
            Quit,
        }
        pub fn start_tray() {
            let mut tray = TrayItem::new("Unified Clipboard", "icon").unwrap();
            tray.add_label("Unified Clipboard").unwrap();
            let (tx, rx) = mpsc::channel();
            tray.add_menu_item("Quit", move || {
                println!("Quit");
                tx.send(Message::Quit).unwrap();
            })
            .unwrap();
            loop {
                match rx.recv() {
                    Ok(Message::Quit) => {
                        process::exit(0);
                    }
                    _ => {}
                }
            }
        }

    } else if #[cfg(target_os = "linux")] {
        pub fn start_tray() {
            gtk::init().unwrap();
            let mut tray = TrayItem::new("Unified Clipboard", "accessories-calculator").unwrap();
            tray.add_label("Unified Clipboard").unwrap();
            tray.add_menu_item("Quit", || {
                gtk::main_quit();
            }).unwrap();

            gtk::main();
        }
    } else if #[cfg(target_os = "macos")] {
        pub fn start_tray() {
            let mut tray = TrayItem::new("Unified Clipboard", "").unwrap();
            tray.add_label("Unified Clipboard").unwrap();
            let mut inner = tray.inner_mut();
            inner.add_quit_item("Quit");
            inner.display();
        }
    }
}
