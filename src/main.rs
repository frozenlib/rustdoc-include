use anyhow::Result;
use ignore::Walk;
use regex::{Captures, Regex};
use std::{
    borrow::Cow,
    ffi::OsStr,
    fs::read,
    fs::write,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

fn main() -> Result<()> {
    let args = Opt::from_args();

    for e in Walk::new(args.dir) {
        let e = e?;
        if let Some(t) = e.file_type() {
            if t.is_file() {
                let path = e.path();
                if path.extension() != Some(OsStr::new("rs")) {
                    continue;
                }
                if let Some(dir) = path.parent() {
                    let input = String::from_utf8(read(&path)?)?;
                    if let Some(s) = apply(&dir, &input) {
                        println!("update : {}", path.display());
                        if !args.dry_run {
                            write(path, s)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
fn apply(dir: &Path, input: &str) -> Option<String> {
    let re = Regex::new(r#"(?ms)(^\s*//\s*#\[include_doc\s*=\s*"([^"]*)"]\s*$).*?(^\s*//\s*#\[include_doc_end\s*=\s*"([^"]*)"\s*\]\s*$)"#).unwrap();
    let s = re.replace_all(input, |c: &Captures| {
        let path_start = c.get(2).unwrap().as_str();
        let path_end = c.get(4).unwrap().as_str();
        if path_start != path_end {
            eprintln!("error : include path was not match.");
            return c.get(0).unwrap().as_str().to_string();
        }
        let mut r = String::new();
        r += c.get(1).unwrap().as_str();
        r += "\n/**\n";
        r += &read_source(dir, path_start);
        r += "\n*/\n";
        r += c.get(3).unwrap().as_str();
        r
    });
    if let Cow::Owned(s) = s {
        Some(s)
    } else {
        None
    }
}
fn read_source(dir: &Path, path: &str) -> String {
    match try_read_source(dir, path) {
        Ok(value) => value,
        Err(e) => format!("ERROR : {}", e),
    }
}
fn try_read_source(dir: &Path, path: &str) -> Result<String> {
    let path = dir.join(path);
    println!("reading {}", path.display());
    Ok(String::from_utf8(read(&path)?)?)
}

#[derive(StructOpt)]
struct Opt {
    #[structopt(parse(from_os_str))]
    dir: PathBuf,

    #[structopt(long = "dry-run")]
    dry_run: bool,
}
