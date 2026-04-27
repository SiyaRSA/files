use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let args: Vec<String> = env::args().collect();
    let release = args.iter().any(|a| a == "clap-release");

    let mut build = Command::new("cargo");
    build.arg("build");

    if release {
        build.arg("--release");
    }

    let status = build.status().expect("failed to run cargo build");
    if !status.success() {
        std::process::exit(1);
    }

    let profile = if release { "release" } else { "debug" };

    let lib_name = if cfg!(target_os = "windows") {
        "rws.dll"
    } else if cfg!(target_os = "macos") {
        "librws.dylib"
    } else {
        "librws.so"
    };

    let target_dir = PathBuf::from("target").join(profile);
    let src = target_dir.join(lib_name);
    let dst = target_dir.join("rws.clap");

    fs::copy(&src, &dst).expect("failed to copy plugin");

    println!("Built {}", dst.display());
}