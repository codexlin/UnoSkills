use std::{env, fs, path::PathBuf};

use nsh_bundler::BundleOptions;

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=windows.rc");
        println!("cargo:rerun-if-changed=../../packaging/icons/unoskills.ico");
        embed_resource::compile_for("windows.rc", ["unoskills-app"], embed_resource::NONE)
            .manifest_required()
            .expect("embed the UnoSkills application icon");
    }

    println!("cargo:rerun-if-changed=ui");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo should set OUT_DIR"));
    let release = env::var("PROFILE").is_ok_and(|profile| profile == "release");
    let bundle = nsh_bundler::bundle(&BundleOptions {
        entry: PathBuf::from("ui/App.tsx"),
        development: !release,
        minify: release,
        split: false,
        ui_sets: nsh_base::registry()
            .sets()
            .into_iter()
            .map(Into::into)
            .collect(),
        ..Default::default()
    })
    .unwrap_or_else(|error| panic!("{}", error.plain));
    fs::write(output.join("ui.js"), bundle.code).expect("write bundled NSH UI");
}
