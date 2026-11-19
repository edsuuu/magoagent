//! macOS: compiles the SwiftUI interface (macos/MagoUI, a SwiftPM static library) and links it with the Swift runtime.
//! Every other system: nothing. The contract is in docs/swiftui.md.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    // read at run time: the compiled build script is shared between checkouts (worktrees) of the same target dir
    let package = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR")).join("macos/MagoUI");
    println!("cargo:rerun-if-changed={}", package.join("Package.swift").display());
    println!("cargo:rerun-if-changed={}", package.join("Sources").display());
    println!("cargo:rerun-if-env-changed=SDKROOT");
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        _ => "x86_64",
    };
    let triple = format!("{arch}-apple-macosx14.0");
    let swift = |extra: &[&str]| {
        let mut command = Command::new("swift");
        command.args(["build", "-c", "release", "--package-path"]).arg(&package).args(["--triple", &triple]).args(extra);
        if let Some(sdk) = pinned_sdk() {
            command.env("SDKROOT", sdk);
        }
        command
    };
    let status = swift(&[]).status().expect("o `swift` não foi encontrado: instale as Command Line Tools (xcode-select --install)");
    assert!(status.success(), "swift build de macos/MagoUI falhou (veja as mensagens acima)");
    let products = output(&mut swift(&["--show-bin-path"]));
    println!("cargo:rustc-link-search=native={products}");
    println!("cargo:rustc-link-lib=static=MagoUI");
    // the Swift runtime: the system's (the SDK's .tbd, loaded from /usr/lib/swift) and the toolchain's compatibility libraries
    let sdk = output(Command::new("xcrun").args(["--sdk", "macosx", "--show-sdk-path"]));
    println!("cargo:rustc-link-search=native={sdk}/usr/lib/swift");
    let swiftc = output(Command::new("xcrun").args(["--find", "swiftc"]));
    if let Some(toolchain) = Path::new(&swiftc).parent().and_then(Path::parent) {
        println!("cargo:rustc-link-search=native={}", toolchain.join("lib/swift/macosx").display());
    }
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    for framework in ["SwiftUI", "AppKit", "Foundation", "Combine", "QuartzCore"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}

/// The SDK the Swift build uses. The Command Line Tools ship no SwiftUIMacros plugin, and from the macOS 27 SDK on
/// `@State` is a macro that needs it: with them alone the build takes the 26 SDK beside it. An SDKROOT given wins.
// ponytail: a fixed SDK name; drop the pin when the Command Line Tools ship the plugin (or with Xcode, which has it)
fn pinned_sdk() -> Option<String> {
    if std::env::var_os("SDKROOT").is_some() {
        return None;
    }
    let developer = output(Command::new("xcode-select").arg("-p"));
    let older = Path::new(&developer).join("SDKs/MacOSX26.sdk");
    (developer.contains("CommandLineTools") && older.exists()).then(|| older.display().to_string())
}

/// What a command prints, trimmed; it failing stops the build.
fn output(command: &mut Command) -> String {
    let result = command.output().unwrap_or_else(|error| panic!("{command:?}: {error}"));
    assert!(result.status.success(), "{command:?} falhou: {}", String::from_utf8_lossy(&result.stderr));
    String::from_utf8_lossy(&result.stdout).trim().to_string()
}
