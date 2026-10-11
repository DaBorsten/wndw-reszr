fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    // Symbol und Versionsangaben der exe, wie sie Explorer und Taskleiste zeigen.
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        winresource::WindowsResource::new()
            .set_icon("assets/icon.ico")
            .set("ProductName", "Wndw Reszr")
            .set("FileDescription", "Wndw Reszr")
            .compile()
            .expect("Ressourcen der exe");
    }
}
