#[cfg(target_os = "windows")]
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    res.set_icon_with_id("assets/icon.ico", "icon");
    res.compile().unwrap();
}

#[cfg(not(target_os = "windows"))]
fn main() {}
