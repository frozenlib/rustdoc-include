use crate::fmt::*;
use crate::text_pos::*;
use anyhow::{bail, Result};
use colored::*;
use ignore::Walk;
use std::{
    ffi::OsStr,
    fs::read,
    fs::write,
    ops::Range,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

mod attr;
mod fmt;
mod text_pos;

use attr::{Attr, BadAttrError};

fn main() {
    if let Err(e) = run() {
        eprintln!("{}: {}", "error".red().bold(), e);
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
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
                        Ok(result) => {
                            if let Some(text) = result.text {
                                eprintln!("{}: {}", "update".green().bold(), rel_path.display());
                                for log in result.logs {
                                    if log.is_modified {
                                        eprintln!("  <-- {}", log.source_rel_path.display());
                                    }
                                }
                                if !args.dry_run {
                                    write(path, text)?;
                                }
                            }
                        }
                        Err(e) => {
                            bail!("{}", e.to_error_message(&rel_path, &input));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn make_pair<'a>(
    start: &mut Option<Attr<'a>>,
    attr: Result<Attr<'a>, BadAttrError>,
) -> Result<Option<(Attr<'a>, Attr<'a>)>, ApplyError<'a>> {
    match attr {
        Ok(attr) => {
            if attr.action == attr::Action::Start {
                if let Some(start) = start.replace(attr) {
                    Err(ApplyError::MissingAttr(start))
                } else {
                    Ok(None)
                }
            } else {
                let end = attr;
                if let Some(start) = start.take() {
                    if let Some(mismatch) = start.mismatch(&end) {
                        Err(ApplyError::MismatchAttr {
                            start,
                            end,
                            mismatch,
                        })
                    } else {
                        Ok(Some((start, end)))
                    }
                } else {
                    Err(ApplyError::MissingAttr(end))
                }
            }
        }
        Err(e) => Err(ApplyError::BadAttr(e)),
    }
}
fn trim<'a, 'b>(
    text: &'a str,
    start: &Attr<'b>,
    end: &Attr<'b>,
) -> Result<&'a str, ApplyError<'b>> {
    let index_start = match start.arg {
        attr::ActionArg::None => 0,
        attr::ActionArg::Line(line) => line_offset(text, line),
        attr::ActionArg::LineRev(line) => line_offset_rev(text, line),
        attr::ActionArg::Text(p) => {
            if let Some(index) = text.find(p) {
                index
            } else {
                return Err(ApplyError::TextNofFound(start.clone()));
            }
        }
    };
    let index_end = match end.arg {
        attr::ActionArg::None => text.len(),
        attr::ActionArg::Line(line) => line_offset(text, line),
        attr::ActionArg::LineRev(line) => line_offset_rev(text, line),
        attr::ActionArg::Text(p) => {
            if let Some(index) = text.rfind(p) {
                index
            } else {
                return Err(ApplyError::TextNofFound(end.clone()));
            }
        }
    };
    let index_start = index_end - text[index_start..index_end].trim_start().len();
    let index_end = index_start + text[index_start..index_end].trim_end().len();
    Ok(&text[index_start..index_end])
}
fn line_offset(text: &str, mut line: usize) -> usize {
    if line <= 1 {
        return 0;
    }
    line -= 1;
    for (index, c) in text.char_indices() {
        if c == '\n' {
            line -= 1;
            if line == 0 {
                return index + 1;
            }
        }
    }
    text.len()
}
fn line_offset_rev(text: &str, mut line: usize) -> usize {
    if line == 0 {
        return text.len();
    }
    for (index, c) in text.char_indices().rev() {
        if c == '\n' {
            line -= 1;
            if line == 0 {
                return index;
            }
        }
    }
    0
}
fn is_modified(text_new: &str, text_old: &str, start: &Attr, end: &Attr) -> bool {
    let old_text = &text_old[start.range.end..end.range.start];
    if !old_text.starts_with("\n") {
        return true;
    }
    text_new != &old_text[1..]
}
fn apply<'a>(root: &Path, base: &Path, input: &'a str) -> Result<ApplyResult, ApplyError<'a>> {
    let mut logs = Vec::new();
    let mut attr_start = None;
    let mut text = String::new();
    let mut text_is_modified = false;
    let mut last_offset = 0;
    for attr in Attr::find_iter(input) {
        if let Some((start, end)) = make_pair(&mut attr_start, attr)? {
            let kind = start.kind;
            text.push_str(&input[last_offset..start.range.end]);
            text.push('\n');
            let source = start.path;
            match include(root, base, source) {
                Ok(s) => {
                    let source_rel_path = s.rel_path;
                    let text_new = to_doc_comment(
                        trim(&s.text, &start, &end)?,
                        start.kind.doc_comment_prefix(),
                    );
                    let is_modified = is_modified(&text_new, input, &start, &end);
                    if is_modified {
                        text_is_modified = is_modified;
                        if let Some(source_range) = Attr::find_may_bad(&text_new) {
                            return Err(ApplyError::SourceContent {
                                attr: start,
                                source_range,
                                source_rel_path,
                                source_text: text_new.to_string(),
                                reason: "source file contains attribute".into(),
                            });
                        }
                    }

                    text.push_str(&text_new);
                    logs.push(LogEntry {
                        source_rel_path,
                        is_modified,
                    });
                }
                Err(e) => {
                    return Err(ApplyError::SourceRead {
                        attr: start,
                        reason: e.to_string(),
                    });
                }
            }
            last_offset = end.range.start;
        }
    }
    text.push_str(&input[last_offset..]);
    let text = if text_is_modified { Some(text) } else { None };
    Ok(ApplyResult { text, logs })
}

struct IncludeResult {
    rel_path: PathBuf,
    text: String,
}

fn include(root: &Path, base: &Path, source: &str) -> Result<IncludeResult> {
    let source = base.join(source);
    if let Ok(rel_path) = source.canonicalize()?.strip_prefix(&root.canonicalize()?) {
        Ok(IncludeResult {
            rel_path: rel_path.to_path_buf(),
            text: String::from_utf8(read(&source)?)?,
        })
    } else {
        bail!("source is out of root");
    }
}
fn to_doc_comment(s: &str, prefix: &str) -> String {
    let mut r = String::new();
    for line in s.lines() {
        r.push_str(prefix);
        r.push_str(line);
        r.push('\n');
    }
    r
}

#[derive(StructOpt)]
struct Opt {
    #[structopt(parse(from_os_str))]
    root: PathBuf,

    #[structopt(long = "dry-run")]
    dry_run: bool,
}

struct ApplyResult {
    text: Option<String>,
    logs: Vec<LogEntry>,
}
struct LogEntry {
    source_rel_path: PathBuf,
    is_modified: bool,
}

enum ApplyError<'a> {
    BadAttr(BadAttrError),
    MissingAttr(Attr<'a>),
    MismatchAttr {
        start: Attr<'a>,
        end: Attr<'a>,
        mismatch: attr::Mismatch,
    },
    TextNofFound(Attr<'a>),
    SourceRead {
        attr: Attr<'a>,
        reason: String,
    },
    SourceContent {
        attr: Attr<'a>,
        source_rel_path: PathBuf,
        source_text: String,
        source_range: Range<usize>,
        reason: String,
    },
}
impl<'a> ApplyError<'a> {
    fn to_error_message(&self, rel_path: &Path, input: &str) -> String {
        match self {
            ApplyError::BadAttr(e) => e.message(rel_path, input),
            ApplyError::MissingAttr(attr) => {
                let msg = match attr.action {
                    attr::Action::Start => "missing end attribute",
                    attr::Action::End => "missing start attribute",
                };
                format!("{}\n{}", msg, attr.message(&rel_path, input))
            }
            ApplyError::MismatchAttr {
                start,
                end,
                mismatch,
            } => {
                let start_line = start.line(input);
                let end_line = end.line(input);
                format!(
                    "{}\n{}\n{}\n{}",
                    mismatch.message(),
                    fmt_link(rel_path, start_line),
                    fmt_link(rel_path, end_line),
                    fmt_source(vec![
                        (start_line, &input[start.range()]),
                        (end_line, &input[end.range()])
                    ])
                )
            }
            ApplyError::TextNofFound(attr) => {
                let msg = match attr.action {
                    attr::Action::Start => "start text not found",
                    attr::Action::End => "end text not found",
                };
                format!("{}\n{}", msg, attr.message(rel_path, input))
            }
            ApplyError::SourceRead { attr, reason } => format!(
                "cannot read `{}` ({})\n{}",
                attr.path,
                reason,
                attr.message(rel_path, input)
            ),
            ApplyError::SourceContent {
                attr,
                source_rel_path,
                source_text,
                source_range,
                reason,
            } => {
                let line = attr.line(input);
                let source_line = to_line(&source_text, source_range.start);
                format!(
                    "{}\n{}\n{}\n{}",
                    reason,
                    fmt_link(rel_path, line),
                    fmt_link(source_rel_path, source_line),
                    fmt_source(vec![("", &source_text[source_range.clone()])])
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::{
        fs::{read, read_dir, DirEntry},
        path::Path,
    };
    #[test]
    fn test_convert_file() -> Result<()> {
        let dir = Path::new("./tests/data");
        for e in read_dir(&dir)? {
            let e = e?;
            if let Some((input, expected)) = to_input_expected(e) {
                eprint!("test {} ... ", input);
                match check_convert_file(dir, &dir.join(input), &dir.join(expected)) {
                    Ok(_) => {
                        eprintln!("{}", "ok".green());
                    }
                    Err(e) => {
                        eprintln!("{}", "FAILED".red());
                        bail!("{}", e)
                    }
                }
            }
        }
        Ok(())
    }
    fn to_input_expected(e: DirEntry) -> Option<(String, String)> {
        if !e.file_type().ok()?.is_file() {
            return None;
        }
        let path = e.path();
        let name = path.file_name()?.to_str()?;
        if !name.ends_with(".rs") || name.ends_with(".expected.rs") {
            return None;
        }
        let mut name_expected = path.file_stem()?.to_str()?.to_string();
        name_expected += ".expected.rs";
        Some((name.to_string(), name_expected))
    }
    fn check_convert_file(dir: &Path, input_path: &Path, expected_path: &Path) -> Result<()> {
        let input_str = String::from_utf8(read(input_path)?)?;
        let expected_str = String::from_utf8(read(expected_path)?)?;
        let input_rel_path = input_path.strip_prefix(&dir).unwrap_or(&input_path);
        match apply(&dir, &dir, &input_str) {
            Ok(x) => {
                let output_str = if let Some(text) = &x.text {
                    text
                } else {
                    &input_str
                };
                let output_str = output_str.trim();
                let expected_str = expected_str.trim();
                if output_str != expected_str {
                    bail!(
                        "mismatch result\nexpected:\n{}\n\nactual:\n{}",
                        expected_str,
                        output_str
                    );
                }
                Ok(())
            }
            Err(e) => {
                bail!("{}", e.to_error_message(input_rel_path, &input_str))
            }
        }
    }
}
