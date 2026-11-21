use std::{env, fs, path::PathBuf};

use nsh_bundler::BundleOptions;

fn main() {
    println!("cargo:rerun-if-changed=ui");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo should set OUT_DIR"));
    let release = env::var("PROFILE").is_ok_and(|profile| profile == "release");
    let bundle = nsh_bundler::bundle(&BundleOptions {
        entry: PathBuf::from("ui/App.tsx"),
        development: !release,
        minify: release,
        split: false,
        ui_sets: vec![nsh_base::registry().into()],
        ..Default::default()
    })
    .unwrap_or_else(|error| panic!("{}", error.plain));
    fs::write(output.join("ui.js"), bundle.code).expect("write bundled NSH UI");
}
