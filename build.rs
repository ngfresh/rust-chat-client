//! 构建脚本：把 assets/icon.ico 嵌入 Windows exe 作为应用图标。

fn main() {
    // 仅在 Windows 目标上嵌入图标资源
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("icon.rc", embed_resource::NONE);
    }
    // 图标或资源文件变化时重新构建
    println!("cargo:rerun-if-changed=icon.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
}
