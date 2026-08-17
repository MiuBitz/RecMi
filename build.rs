fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set("ProductName", "RecMi Desktop Screen Recorder");
        res.set("FileDescription", "RecMi Screen Recorder");
        res.set("CompanyName", "RecMi Project");
        res.set("LegalCopyright", "Copyright © 2026 RecMi");
        res.compile().unwrap();
    }
}
