use std::env;
use std::path::PathBuf;

fn main() {
    tauri_build::build();
    // 测试二进制需要自己的 manifest：声明 comctl32 v6，否则启动即缺 TaskDialogIndirect 入口（见 tests/windows.manifest）
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("tests/windows.manifest");
    println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}", manifest.display());
}
