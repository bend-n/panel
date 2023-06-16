#![feature(utf8_chunks)]
use std::fs;
use std::io::prelude::*;
use std::path::Path;

use minify_html::{minify, Cfg};

/// like [String::from_utf8_lossy] but instead of being lossy it panics
pub fn from_utf8(v: &[u8]) -> &str {
    let mut iter = std::str::Utf8Chunks::new(v);
    if let Some(chunk) = iter.next() {
        let valid = chunk.valid();
        if chunk.invalid().is_empty() {
            debug_assert_eq!(valid.len(), v.len());
            return valid;
        }
    } else {
        return "";
    };
    unreachable!("invalid utf8")
}

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
    let minified = from_utf8(&minified);
    let minified = minified.replace(
        "ws://localhost:4001/connect/",
        &format!(
            "{}",
            std::env::var("URL").unwrap_or("ws://localhost:4001/connect/".to_string())
        ),
    );
    f.write_all(minified.as_bytes())
}

fn main() -> std::io::Result<()> {
    if !Path::new("html").exists() {
        std::fs::create_dir("html")?;
    }

    for path in fs::read_dir("html-src")? {
        process(path.unwrap().path().file_name().unwrap())?;
    }
    println!("cargo:rerun-if-changed=html-src/");
    println!("cargo:rerun-if-changed=build.rs");
    Ok(())
}
