//! 安卓音频输出经 cpal 使用 oboe（C++ 库），但 oboe-sys 不会链接 C++ 运行时，
//! 产物能编译通过，加载时却因找不到 `__cxa_pure_virtual` 等符号而崩溃。
//! 这里静态链接 NDK 的 libc++：应用里只有这一个原生库用到 C++，无需另外打包 libc++_shared.so。

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let android = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "android");
    let device = std::env::var_os("CARGO_FEATURE_DEVICE").is_some();
    if android && device {
        println!("cargo:rustc-link-lib=c++_static");
        println!("cargo:rustc-link-lib=c++abi");
    }
}
