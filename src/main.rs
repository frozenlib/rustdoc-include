use anyhow::{bail, Result};
use ignore::Walk;
use parse_display::Display;
use regex::{Captures, Regex};
use std::{
    ffi::OsStr,
    fs::read,
    fs::write,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

fn main() -> Result<()> {
    let args = Opt::from_args();
    for e in Walk::new(&args.root) {
        let e = e?;
        if let Some(t) = e.file_type() {
            if t.is_file() {
                let path = e.path();
                if path.extension() != Some(OsStr::new("rs")) {
                    continue;
                }
                let rel_path = path.strip_prefix(&args.root).unwrap_or(path);
                if let Some(base) = path.parent() {
                    let input = String::from_utf8(read(&path)?)?;
                    match apply(&args.root, base, &input) {
                        ApplyResult::Ok { text, logs } => {
                            if let Some(text) = text {
                                eprintln!("update : {}", rel_path.display());
                                for log in logs {
                                    if log.is_modified {
                                        eprintln!("  <-- {}", log.source);
                                    }
                                }
                                if !args.dry_run {
                                    write(path, text)?;
                                }
                            }
                        }
                        ApplyResult::Error { errors } => {
                            for e in errors {
                                eprintln!("{}", e.to_error_message(&rel_path, &input));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
fn apply(root: &Path, base: &Path, input: &str) -> ApplyResult {
    let re = Regex::new(r#"(?ms)(^\s*//\s*#\[include_doc\s*=\s*"([^"]*)"]\s*$).*?(^\s*//\s*#\[include_doc_end\s*=\s*"([^"]*)"\s*\]\s*$)"#).unwrap();
    let mut logs = Vec::new();
    let mut errors = Vec::new();
    let output = re.replace_all(input, |c: &Captures| {
        let start_source = c.get(2).unwrap();
        let end_source = c.get(4).unwrap();
        if start_source.as_str() != end_source.as_str() {
            errors.push(ErrorEntry::MismatchSource {
                start_offset: start_source.start(),
                start_source: start_source.as_str().into(),
                end_offset: end_source.start(),
                end_source: end_source.as_str().into(),
            });
        }
        if !errors.is_empty() {
            return String::new();
        }

        let source = start_source.as_str().to_string();
        let offset = start_source.start();
        let mut text = String::new();
        text += c.get(1).unwrap().as_str();
        text += "\n/**\n";
        match include(root, base, &source) {
            Ok(s) => text += &s,
            Err(e) => {
                errors.push(ErrorEntry::ReadSource {
                    source,
                    offset,
                    reason: e.to_string(),
                });
                return String::new();
            }
        }
        text += "\n*/\n";
        text += c.get(3).unwrap().as_str();
        let is_modified = text != c.get(0).unwrap().as_str();
        logs.push(LogEntry {
            _offset: offset,
            is_modified,
            source,
        });
        text
    });
    if !errors.is_empty() {
        ApplyResult::Error { errors }
    } else {
        let text = if output == input {
            None
        } else {
            Some(output.into())
        };
        ApplyResult::Ok { logs, text }
    }
}

fn include(root: &Path, base: &Path, source: &str) -> Result<String> {
    let source = base.join(source);
    if source.canonicalize()?.starts_with(&root.canonicalize()?) {
        Ok(String::from_utf8(read(&source)?)?)
    } else {
        bail!("source is out of root");
    }
}

#[derive(StructOpt)]
struct Opt {
    #[structopt(parse(from_os_str))]
    root: PathBuf,

    #[structopt(long = "dry-run")]
    dry_run: bool,
}

enum ApplyResult {
    Ok {
        text: Option<String>,
        logs: Vec<LogEntry>,
    },
    Error {
        errors: Vec<ErrorEntry>,
    },
}
struct LogEntry {
    _offset: usize,
    source: String,
    is_modified: bool,
}

enum ErrorEntry {
    MismatchSource {
        start_offset: usize,
        start_source: String,
        end_offset: usize,
        end_source: String,
    },
    ReadSource {
        offset: usize,
        source: String,
        reason: String,
    },
}
impl ErrorEntry {
    fn to_error_message(&self, rel_path: &Path, input: &String) -> String {
        match self {
            ErrorEntry::MismatchSource {
                start_offset,
                start_source,
                end_offset,
                end_source,
            } => {
                let start_pos = TextPos::from_str_offset(input, *start_offset);
                let end_pos = TextPos::from_str_offset(input, *end_offset);
                format!(
                    r"error : mismatch source.
  start : `{}` ({}:{})
    end : `{}` ({}:{})
",
                    start_source,
                    rel_path.display(),
                    start_pos,
                    end_source,
                    rel_path.display(),
                    end_pos,
                )
            }
            ErrorEntry::ReadSource {
                offset,
                source,
                reason,
            } => {
                let pos = TextPos::from_str_offset(input, *offset);
                format!(
                    r"error : read source failed. `{}` ({})
--> {}:{}",
                    source,
                    reason,
                    rel_path.display(),
                    pos
                )
            }
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Display)]
#[display("{line}:{column}")]
struct TextPos {
    line: usize,
    column: usize,
}
impl TextPos {
    fn from_str_offset(s: &str, offset: usize) -> Self {
        let mut value = Self { line: 1, column: 1 };
        for (index, c) in s.char_indices() {
            if index >= offset {
                break;
            }
            if c == '\n' {
                value.line += 1;
                value.column = 1;
            } else {
                value.column += 1;
            }
        }
        value
    }
}

#[test]
fn text_pos_from_str_offset() {
    let s = "abc\ndef";
    check(s, 0, 1, 1);
    check(s, 1, 1, 2);
    check(s, 2, 1, 3);
    check(s, 3, 1, 4);
    check(s, 4, 2, 1);
    check(s, 5, 2, 2);
    fn check(s: &str, offset: usize, line: usize, column: usize) {
        assert_eq!(
            TextPos::from_str_offset(s, offset),
            TextPos { line, column }
        );
    }
}
