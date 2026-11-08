fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("app.ico");
        res.set("ProductName", "BandPill");
        res.set(
            "FileDescription",
            "BandPill - Windows 11 Bandwidth Tracker & Anti-Leak Shield",
        );
        res.set("LegalCopyright", "Copyright (c) 2026");
        res.set("OriginalFilename", "BandPill.exe");
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to compile Windows resource: {}", e);
        }
    }
}
