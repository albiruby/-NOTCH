fn main() {
    // Embeds assets/notch.ico as resource 1: the exe icon in Explorer and the
    // icon the tray loads at runtime.
    println!("cargo:rerun-if-changed=assets/notch.rc");
    println!("cargo:rerun-if-changed=assets/notch.ico");
    embed_resource::compile("assets/notch.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
