use crate::fmt::*;
use crate::text_pos::*;
use colored::Colorize;
use once_cell::sync::Lazy;
use regex::{Captures, Match, Regex};
use std::{ops::Range, path::Path};
use thiserror::Error;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Attr<'a> {
    pub range: Range<usize>,
    pub path: &'a str,
    pub kind: Kind,
    pub action: Action,
    pub arg: ActionArg<'a>,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum Kind {
    Inner,
    Outer,
}
impl Kind {
    pub fn doc_comment_prefix(self) -> &'static str {
        match self {
            Kind::Inner => "//! ",
            Kind::Outer => "/// ",
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum Action {
    Start,
    End,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum ActionArg<'a> {
    None,
    Line(usize),
    LineRev(usize),
    Text(&'a str),
}

pub enum Mismatch {
    Kind,
    Path,
}
impl Mismatch {
    pub fn message(&self) -> &'static str {
        match self {
            Mismatch::Kind => "mismatch attribute kind.",
            Mismatch::Path => "mismatch include path.",
        }
    }
}

static RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?m:^[ \t]*//[ \t]*#(!?)\[[ \t]*include_doc(?:[ \t]*\([ \t]*"([^"]*)"[ \t]*,[ \t]*(start|end)[ \t]*(?:\([ \t]*(?:"([^"]*)"|(-)?([0-9]+))[ \t]*\)[ \t]*)?\)[ \t]*|.*)\][ \t]*$)"#,
    )
    .unwrap()
});

impl<'a> Attr<'a> {
    pub fn from_captures(c: &Captures<'a>) -> Option<Self> {
        let target = match c.get(1)?.as_str() {
            "" => Kind::Outer,
            "!" => Kind::Inner,
            _ => unreachable!(),
        };
        let path = c.get(2)?.as_str();
        let kind = match c.get(3)?.as_str() {
            "start" => Action::Start,
            "end" => Action::End,
            _ => unreachable!(),
        };
        let arg = if let Some(c) = c.get(4) {
            ActionArg::Text(c.as_str())
        } else if let Some(c5) = c.get(6) {
            let value = c5.as_str().parse().ok()?;
            if c.get(5).is_some() {
                ActionArg::LineRev(value)
            } else {
                ActionArg::Line(value)
            }
        } else {
            ActionArg::None
        };
        Some(Self {
            range: c.get(0)?.range(),
            kind: target,
            path,
            action: kind,
            arg,
        })
    }
    pub fn mismatch(&self, other: &Self) -> Option<Mismatch> {
        if self.kind != other.kind {
            Some(Mismatch::Kind)
        } else if self.path != other.path {
            Some(Mismatch::Path)
        } else {
            None
        }
    }
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    pub fn find_iter(text: &'a str) -> impl Iterator<Item = Result<Attr, BadAttrError>> {
        RE.captures_iter(text).map(|c| {
            Self::from_captures(&c).ok_or_else(|| BadAttrError::from_match(c.get(0).unwrap()))
        })
    }
    pub fn find_may_bad(text: &str) -> Option<Range<usize>> {
        Some(RE.find(text)?.range())
    }

    pub fn message(&self, rel_path: &Path, input: &str) -> String {
        format!(
            "{}\n{}",
            fmt_link(rel_path, self.line(input)),
            fmt_source(vec![("", &input[self.range()])]),
        )
    }
    pub fn line(&self, input: &str) -> usize {
        to_line(input, self.range.start)
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
#[error("invalid attribute.")]
pub struct BadAttrError {
    range: Range<usize>,
}
impl BadAttrError {
    fn from_match(m: Match) -> Self {
        Self { range: m.range() }
    }
    pub fn message(&self, rel_path: &Path, input: &str) -> String {
        let p = TextPos::from_str_offset(input, self.range.start);
        format!(
            r"invalid attribute
{}
 {} {}",
            fmt_link(rel_path, p.line),
            "|".cyan().bold(),
            &input[self.range()]
        )
    }
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn attr_check(s: &str, kind: Kind, path: &str, action: Action, arg: ActionArg) {
        let expected = Attr {
            range: 0..s.len(),
            kind,
            path,
            action,
            arg,
        };
        let c = RE.captures(s).expect(&format!("not match `{}`", s));
        let value = Attr::from_captures(&c).expect("cannot crate attr from capture");
        assert_eq!(value, expected, "input = `{}`", s);
    }

    #[test]
    fn attr_outer() {
        attr_check(
            r#"// #[include_doc("abc",start)]"#,
            Kind::Outer,
            "abc",
            Action::Start,
            ActionArg::None,
        );
    }
    #[test]
    fn attr_inner() {
        attr_check(
            r#"// #![include_doc("abc",start)]"#,
            Kind::Inner,
            "abc",
            Action::Start,
            ActionArg::None,
        );
    }
    #[test]
    fn attr_start() {
        attr_check(
            r#"// #[include_doc("abc",start)]"#,
            Kind::Outer,
            "abc",
            Action::Start,
            ActionArg::None,
        );
    }
    #[test]
    fn attr_end() {
        attr_check(
            r#"// #[include_doc("abc",end)]"#,
            Kind::Outer,
            "abc",
            Action::End,
            ActionArg::None,
        );
    }

    #[test]
    fn attr_arg_none() {
        attr_check(
            r#"// #[include_doc("abc",start)]"#,
            Kind::Outer,
            "abc",
            Action::Start,
            ActionArg::None,
        );
    }
    #[test]
    fn attr_arg_text() {
        attr_check(
            r#"// #[include_doc("abc",start("this is text"))]"#,
            Kind::Outer,
            "abc",
            Action::Start,
            ActionArg::Text("this is text"),
        );
    }
    #[test]
    fn attr_arg_offset() {
        attr_check(
            r#"// #[include_doc("abc",start(10))]"#,
            Kind::Outer,
            "abc",
            Action::Start,
            ActionArg::Line(10),
        );
    }
    #[test]
    fn attr_arg_offset_end() {
        attr_check(
            r#"// #[include_doc("abc",start(-10))]"#,
            Kind::Outer,
            "abc",
            Action::Start,
            ActionArg::LineRev(10),
        );
    }

    #[test]
    fn attr_space_arg_none() {
        attr_check(
            r#"  //   #[  include_doc  (  "abc"  ,  start  )  ]  "#,
            Kind::Outer,
            "abc",
            Action::Start,
            ActionArg::None,
        );
    }
    #[test]
    fn attr_space_arg_text() {
        attr_check(
            r#"  //   #[  include_doc  (  "abc"  ,  start  (  "this is text"  )  )  ]  "#,
            Kind::Outer,
            "abc",
            Action::Start,
            ActionArg::Text("this is text"),
        );
    }

    fn check_find_iter(text: &str, expected: Vec<Result<Attr, BadAttrError>>) {
        let items: Vec<_> = Attr::find_iter(text).collect();
        assert_eq!(items, expected);
    }

    #[test]
    fn find_attr_1() {
        check_find_iter(
            r#"
// #[include_doc("abc", start)]
"#,
            vec![Ok(Attr {
                range: 1..32,
                kind: Kind::Outer,
                path: "abc",
                action: Action::Start,
                arg: ActionArg::None,
            })],
        );
    }

    #[test]
    fn find_attr_2() {
        check_find_iter(
            r#"
// #[include_doc("abc", start)]
// #[include_doc("abc", end)]
"#,
            vec![
                Ok(Attr {
                    range: 1..32,
                    kind: Kind::Outer,
                    path: "abc",
                    action: Action::Start,
                    arg: ActionArg::None,
                }),
                Ok(Attr {
                    range: 33..62,
                    kind: Kind::Outer,
                    path: "abc",
                    action: Action::End,
                    arg: ActionArg::None,
                }),
            ],
        );
    }

    #[test]
    fn find_attr_error() {
        check_find_iter(
            r#"
// #[include_doc("abc", unknown)]
"#,
            vec![Err(BadAttrError { range: 1..34 })],
        );
    }
    #[test]
    fn find_attr_error2() {
        check_find_iter(
            r#"
// #[include_doc("abc", unknown)]
// #[include_doc("abc", unknown)]
"#,
            vec![
                Err(BadAttrError { range: 1..34 }),
                Err(BadAttrError { range: 35..68 }),
            ],
        );
    }
}
