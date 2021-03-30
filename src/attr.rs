use std::ops::Range;

use once_cell::sync::Lazy;
use regex::{Captures, Match, Regex};
use thiserror::Error;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Attr<'a> {
    range: Range<usize>,
    path: &'a str,
    kind: Kind,
    action: Action,
    arg: ActionArg<'a>,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum Kind {
    Inner,
    Outer,
}
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum Action {
    Start,
    End,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum ActionArg<'a> {
    None,
    Offset(usize),
    OffsetEnd(usize),
    Text(&'a str),
}

static RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?ms:^\s*//\s*#(!?)\[\s*include_doc\s*\(\s*"([^"]*)"\s*,\s*(start|end)\s*(?:\(\s*(?:"([^"]*)"|(-)?([0-9]+))\s*\)\s*)?\)\s*\]\s*$)"#,
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
                ActionArg::OffsetEnd(value)
            } else {
                ActionArg::Offset(value)
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
    pub fn find_iter(text: &'a str) -> impl Iterator<Item = Result<Attr, BadAttrError>> {
        RE.captures_iter(text).map(|c| {
            Self::from_captures(&c).ok_or_else(|| BadAttrError::from_match(c.get(0).unwrap()))
        })
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
            ActionArg::Offset(10),
        );
    }
    #[test]
    fn attr_arg_offset_end() {
        attr_check(
            r#"// #[include_doc("abc",start(-10))]"#,
            Kind::Outer,
            "abc",
            Action::Start,
            ActionArg::OffsetEnd(10),
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
// #[include_doc("abc",end)]        
"#,
            vec![Ok(Attr {
                range: 0..0,
                kind: Kind::Outer,
                path: "abc",
                action: Action::End,
                arg: ActionArg::None,
            })],
        );
    }
}
