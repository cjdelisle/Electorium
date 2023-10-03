// SPDX-License-Identifier: MIT OR ISC
use anyhow::Result;

fn main() -> Result<()> {
    println!("Generating rffi");
    let mut conf = cbindgen::Config::default();
    conf.language = cbindgen::Language::C;
    conf.autogen_warning =
        Some("// This file is generated from src/rffi.rs using cbindgen".to_owned());
    conf.style = cbindgen::Style::Type;
    conf.include_guard = Some("electorium_fuzzable_H".to_owned());
    conf.no_includes = true;
    conf.includes = vec!["stdint.h".to_owned()];
    cbindgen::Builder::new()
        .with_src("./src/lib.rs")
        .with_config(conf)
        .generate()
        .expect("Unable to generate rffi")
        .write_to_file("electorium_fuzzable.h");
    println!("Generating rffi done");
    Ok(())
}