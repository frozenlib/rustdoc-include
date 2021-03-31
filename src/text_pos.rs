use parse_display::Display;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Display)]
#[display("{line}:{column}")]
pub struct TextPos {
    pub line: usize,
    pub column: usize,
}
impl TextPos {
    pub fn from_str_offset(s: &str, offset: usize) -> Self {
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
