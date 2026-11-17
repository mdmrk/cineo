fn main() {
    fastframe_i18n::build::compile_catalogs("assets/i18n");
    println!("cargo::rerun-if-changed=assets/brand/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/brand/icon.ico");
        if let Err(err) = resource.compile() {
            panic!("could not embed the Windows icon: {err}");
        }
    }
}
