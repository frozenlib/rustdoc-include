use colored::Colorize;
use std::fmt::Display;
use std::fmt::Write;
use std::path::Path;

pub fn fmt_link(rel_path: &Path, line: usize) -> String {
    format!("--> {}:{}", rel_path.display(), line)
}
pub fn fmt_source<'a, L: Display>(lines: impl IntoIterator<Item = (L, &'a str)>) -> String {
    let lines: Vec<_> = lines
        .into_iter()
        .map(|(line, content)| (line.to_string(), content))
        .collect();
    let max_width = lines.iter().map(|(line, _)| line.len()).max().unwrap_or(0);
    let mut s = String::new();
    let sep = "|".cyan().bold();
    for (index, (line, content)) in lines.into_iter().enumerate() {
        if index != 0 {
            s.push('\n');
        }
        s.push(' ');
        if max_width != 0 {
            for _ in line.len()..max_width {
                s.push(' ');
            }
            s.push_str(&line);
            s.push(' ');
        }
        write!(&mut s, "{}", sep).unwrap();
        s.push(' ');
        s.push_str(content);
    }
    s
}
