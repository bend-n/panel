use std::fs;
use std::io::prelude::*;
use std::path::Path;

use minify_html::{minify, Cfg};

pub fn process(input: impl AsRef<Path>) -> std::io::Result<()> {
    let mut f = fs::File::create(dbg!(Path::new("html").join(input.as_ref()))).unwrap();
    let mut buf = vec![];
    fs::File::open(Path::new("html-src").join(input.as_ref()))?.read_to_end(&mut buf)?;
    let minified = minify(
        &buf,
        &Cfg {
            minify_js: true,
            minify_css: true,
            ..Default::default()
        },
    );
    f.write_all(&minified)
}

fn main() -> std::io::Result<()> {
    if !Path::new("html").exists() {
        fs::create_dir("html")?;
    }

    for path in fs::read_dir("html-src")? {
        process(path.unwrap().path().file_name().unwrap())?;
    }
    println!("cargo:rerun-if-changed=html-src/");
    println!("cargo:rerun-if-changed=build.rs");
    Ok(())
}
