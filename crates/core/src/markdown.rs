//! The Markdown of a model's answer → blocks to draw: headings, paragraphs,
//! lists (nested by indentation, with checkboxes), fenced code, quotes,
//! rules and pipe tables; inline, code spans, emphasis, strike-through and
//! links. The rules follow quiet.md's Markdown reader (github.com/s1dashu/quiet-md,
//! MIT). What does not close (an answer is drawn while it streams) stays as
//! written, and a citation like `[P3 p.5 "…"]` is ordinary text.

use std::ops::Range;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: Option<String>,
}

/// Text with marks: `runs` are (byte length, marks), cover `text` exactly,
/// and neighbours differ.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rich {
    pub text: String,
    pub runs: Vec<(usize, Marks)>,
}

impl Rich {
    pub fn plain(text: &str) -> Rich {
        let mut rich = Rich::default();
        rich.push(text, Marks::default());
        rich
    }

    fn push(&mut self, text: &str, marks: Marks) {
        if text.is_empty() { return }
        self.text.push_str(text);
        match self.runs.last_mut() {
            Some((len, last)) if *last == marks => *len += text.len(),
            _ => self.runs.push((text.len(), marks)),
        }
    }

    fn append(&mut self, other: Rich) {
        let mut at = 0;
        for (len, marks) in other.runs {
            self.push(&other.text[at..at + len], marks);
            at += len;
        }
    }

    fn mark(mut self, set: impl Fn(&mut Marks)) -> Rich {
        for (_, marks) in &mut self.runs { set(marks) }
        self
    }

    /// Each run as its byte range in `text`.
    pub fn spans(&self) -> impl Iterator<Item = (Range<usize>, &Marks)> {
        let mut at = 0;
        self.runs.iter().map(move |(len, marks)| {
            at += len;
            (at - len..at, marks)
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    Bullet,
    Number(u32),
    Todo(bool),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    /// Lines that followed each other without a blank one, joined by `\n`.
    Paragraph(Rich),
    Heading(u8, Rich),
    Item { marker: Marker, text: Rich, children: Vec<Block> },
    Code { lang: String, text: String },
    Quote(Vec<Block>),
    Rule,
    /// The first row is the header; every row has one cell per column.
    Table { aligns: Vec<Align>, rows: Vec<Vec<Rich>> },
}

pub fn parse(text: &str) -> Vec<Block> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text.trim_end_matches('\n').split('\n').collect();
    blocks(&lines)
}

fn indent_of(line: &str) -> usize {
    let mut n = 0;
    for c in line.chars() {
        match c {
            ' ' => n += 1,
            '\t' => n += 4 - n % 4,
            _ => break,
        }
    }
    n
}

/// `line` without its first `n` columns of indentation.
fn dedent(line: &str, n: usize) -> &str {
    let mut col = 0;
    for (i, c) in line.char_indices() {
        if col >= n { return &line[i..] }
        match c {
            ' ' => col += 1,
            '\t' => col += 4 - col % 4,
            _ => return &line[i..],
        }
    }
    ""
}

/// A list item line: (marker, content column, content).
fn list_item(line: &str) -> Option<(Marker, usize, &str)> {
    let rest = line.trim_start_matches([' ', '\t']);
    let (marker, len) = if let Some(c) = rest.chars().next().filter(|c| matches!(c, '-' | '*' | '+' | '•')) {
        (Marker::Bullet, c.len_utf8())
    } else {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || digits > 9 { return None }
        let dot = rest[digits..].chars().next().filter(|c| matches!(c, '.' | ')'))?;
        (Marker::Number(rest[..digits].parse().ok()?), digits + dot.len_utf8())
    };
    let content = rest[len..].strip_prefix([' ', '\t'])?;
    // `***` and `- - -` are rules, not items.
    if rule(rest) { return None }
    Some((marker, indent_of(line) + len + 1, content))
}

fn rule(line: &str) -> bool {
    let marks: Vec<char> = line.trim().chars().filter(|c| *c != ' ').collect();
    marks.len() >= 3 && matches!(marks[0], '-' | '*' | '_') && marks.iter().all(|c| *c == marks[0])
}

fn fence(line: &str) -> Option<(char, usize, &str)> {
    let t = line.trim_start();
    let c = t.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let n = t.chars().take_while(|&x| x == c).count();
    (n >= 3).then(|| (c, n, t[n..].trim()))
}

fn todo(s: &str) -> Option<(bool, &str)> {
    [("[ ] ", false), ("[x] ", true), ("[X] ", true)].iter().find_map(|(prefix, done)| s.strip_prefix(prefix).map(|rest| (*done, rest)))
}

fn quote_line(s: &str) -> Option<&str> {
    let rest = s.strip_prefix('>')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

fn blocks(lines: &[&str]) -> Vec<Block> {
    let mut out: Vec<Block> = vec![];
    // Lines of the paragraph being read.
    let mut para: Vec<&str> = vec![];
    fn close(para: &mut Vec<&str>, out: &mut Vec<Block>) {
        if !para.is_empty() { out.push(Block::Paragraph(inline(&para.join("\n")))) }
        para.clear();
    }
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            close(&mut para, &mut out);
            i += 1;
            continue;
        }
        let ind = indent_of(line);
        let body = line.trim();
        // Fenced code, closed or (still streaming) running to the end.
        if let Some((mark, n, info)) = fence(line) {
            close(&mut para, &mut out);
            let mut code = vec![];
            let mut j = i + 1;
            while j < lines.len() {
                if fence(lines[j]).is_some_and(|(mark2, n2, rest)| mark2 == mark && n2 >= n && rest.is_empty()) { break }
                code.push(dedent(lines[j], ind));
                j += 1;
            }
            out.push(Block::Code { lang: info.split_whitespace().next().unwrap_or("").to_string(), text: code.join("\n") });
            i = j + 1;
            continue;
        }
        if let Some((marker, col, content)) = list_item(line) {
            close(&mut para, &mut out);
            // The item's own lines: anything indented past its marker.
            let mut sub: Vec<&str> = vec![];
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    // A blank line ends the item unless indented content follows.
                    if lines.get(j + 1).is_some_and(|next| !next.trim().is_empty() && indent_of(next) > ind) {
                        j += 1;
                        continue;
                    }
                    break;
                }
                if indent_of(l) <= ind { break }
                sub.push(dedent(l, col.min(indent_of(l))));
                j += 1;
            }
            let (marker, text) = match (todo(content), marker) {
                (Some((done, rest)), _) => (Marker::Todo(done), rest),
                // Items all written `1.` count on from the one before.
                (None, Marker::Number(1)) => match out.last() {
                    Some(Block::Item { marker: Marker::Number(before), .. }) => (Marker::Number(before + 1), content),
                    _ => (marker, content),
                },
                (None, marker) => (marker, content),
            };
            out.push(Block::Item { marker, text: inline(text.trim()), children: blocks(&sub) });
            i = j;
            continue;
        }
        let hashes = body.bytes().take_while(|&b| b == b'#').count();
        if (1..=6).contains(&hashes) && body[hashes..].starts_with([' ', '\t']) {
            close(&mut para, &mut out);
            out.push(Block::Heading(hashes as u8, inline(body[hashes..].trim().trim_end_matches('#').trim_end())));
            i += 1;
            continue;
        }
        if rule(body) {
            close(&mut para, &mut out);
            out.push(Block::Rule);
            i += 1;
            continue;
        }
        if quote_line(body).is_some() {
            close(&mut para, &mut out);
            let mut sub = vec![];
            while let Some(rest) = lines.get(i).and_then(|l| quote_line(l.trim())) {
                sub.push(rest);
                i += 1;
            }
            out.push(Block::Quote(blocks(&sub)));
            continue;
        }
        if let Some((table, used)) = pipe_table(&lines[i..]) {
            close(&mut para, &mut out);
            out.push(table);
            i += used;
            continue;
        }
        para.push(line.trim_end());
        i += 1;
    }
    close(&mut para, &mut out);
    out
}

fn table_cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').filter(|x| !x.ends_with('\\')).unwrap_or(t);
    let mut cells = vec![];
    let mut cell = String::new();
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cell.push('|');
                chars.next();
            }
            '|' => cells.push(std::mem::take(&mut cell).trim().to_string()),
            _ => cell.push(c),
        }
    }
    cells.push(cell.trim().to_string());
    cells
}

/// A pipe table at the start of `lines`: the table, and the lines it took.
fn pipe_table(lines: &[&str]) -> Option<(Block, usize)> {
    let (head, sep) = (lines.first()?, lines.get(1)?);
    if !head.contains('|') || !sep.contains('-') { return None }
    let seps = table_cells(sep);
    if !seps.iter().all(|cell| !cell.trim_matches(':').is_empty() && cell.trim_matches(':').chars().all(|c| c == '-')) { return None }
    let header = table_cells(head);
    if header.len() != seps.len() { return None }
    let aligns = seps.iter().map(|cell| match (cell.starts_with(':'), cell.ends_with(':')) {
        (true, true) => Align::Center,
        (false, true) => Align::Right,
        _ => Align::Left,
    });
    let mut used = 2;
    let mut rows = vec![header];
    while let Some(line) = lines.get(used).filter(|l| !l.trim().is_empty() && l.contains('|')) {
        rows.push(table_cells(line));
        used += 1;
    }
    let columns = seps.len();
    // `<br>` is how a cell holds two lines.
    let rows = rows.iter().map(|row| (0..columns).map(|c| inline(&row.get(c).map_or(String::new(), |cell| cell.replace("<br>", "\n").replace("<br/>", "\n")))).collect()).collect();
    Some((Block::Table { aligns: aligns.collect(), rows }, used))
}

fn is_boundary(c: Option<char>) -> bool {
    c.is_none_or(|c| c.is_whitespace() || c.is_ascii_punctuation() || !c.is_ascii())
}

fn link_ok(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    ["http://", "https://", "mailto:"].iter().any(|scheme| lower.starts_with(scheme))
}

/// Inline Markdown: escapes, code spans, links, bare URLs and emphasis.
pub fn inline(s: &str) -> Rich {
    let mut out = Rich::default();
    let mut plain = String::new();
    fn flush(plain: &mut String, out: &mut Rich) {
        out.push(plain, Marks::default());
        plain.clear();
    }
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        let c = rest.chars().next().unwrap();
        let prev = s[..i].chars().next_back();
        // `\*`, `\_` …: the character itself.
        if c == '\\' && let Some(next) = rest[1..].chars().next().filter(char::is_ascii_punctuation) {
            plain.push(next);
            i += 1 + next.len_utf8();
            continue;
        }
        if c == '`' {
            let n = rest.bytes().take_while(|&b| b == b'`').count();
            let delim = &rest[..n];
            if let Some(end) = find_run(&rest[n..], delim, b'`') {
                let inner = &rest[n..n + end];
                let inner = if inner.len() > 2 && inner.starts_with(' ') && inner.ends_with(' ') { &inner[1..inner.len() - 1] } else { inner };
                flush(&mut plain, &mut out);
                out.push(inner, Marks { code: true, ..Marks::default() });
                i += n + end + n;
                continue;
            }
            plain.push_str(delim);
            i += n;
            continue;
        }
        if c == '[' && let Some((label, url, used)) = link_at(rest) {
            flush(&mut plain, &mut out);
            let label = if label.is_empty() { Rich::plain(url) } else { inline(label) };
            out.append(label.mark(|m| m.link = Some(url.to_string())));
            i += used;
            continue;
        }
        // A bare URL is a link to itself.
        if c == 'h' && !prev.is_some_and(|p| p.is_alphanumeric()) && (rest.starts_with("https://") || rest.starts_with("http://")) {
            let end = rest.find(|c: char| c.is_whitespace() || "<>\"'`".contains(c) || !c.is_ascii()).unwrap_or(rest.len());
            let url = rest[..end].trim_end_matches(|c: char| ".,;:!?)]}*_~".contains(c));
            if url.split_once("://").is_some_and(|(_, host)| host.len() > 3 && host.contains('.')) {
                flush(&mut plain, &mut out);
                out.push(url, Marks { link: Some(url.to_string()), ..Marks::default() });
                i += url.len();
                continue;
            }
        }
        if matches!(c, '*' | '_' | '~') {
            let n = rest.bytes().take_while(|&b| b == c as u8).count();
            let run = &rest[..n];
            let set: Option<fn(&mut Marks)> = match (c, n) {
                ('*' | '_', 3) => Some(|m| (m.bold, m.italic) = (true, true)),
                ('*' | '_', 2) => Some(|m| m.bold = true),
                ('*' | '_', 1) => Some(|m| m.italic = true),
                ('~', 2) => Some(|m| m.strike = true),
                _ => None,
            };
            // `snake_case` is not emphasis: `_` counts only at the edge of a word.
            let bounded = c == '_';
            if let Some(set) = set
                && (!bounded || is_boundary(prev))
                && let Some(end) = find_close(&rest[n..], c, n)
            {
                let inner = &rest[n..n + end];
                let after = rest[n + end + n..].chars().next();
                if !inner.starts_with(char::is_whitespace) && !inner.ends_with(char::is_whitespace) && (!bounded || is_boundary(after)) {
                    flush(&mut plain, &mut out);
                    out.append(inline(inner).mark(set));
                    i += n + end + n;
                    continue;
                }
            }
            plain.push_str(run);
            i += n;
            continue;
        }
        plain.push(c);
        i += c.len_utf8();
    }
    flush(&mut plain, &mut out);
    out
}

/// Where in `s` a run of `ch` exactly as long as `delim` stands (not part of
/// a longer run, and not at the very start: what it closes is not empty).
fn find_run(s: &str, delim: &str, ch: u8) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut from = 0;
    while let Some(found) = s[from..].find(delim) {
        let at = from + found;
        if at > 0 && bytes[at - 1] != ch && bytes.get(at + delim.len()) != Some(&ch) { return Some(at) }
        from = at + delim.len().max(1);
        while bytes.get(from) == Some(&ch) { from += 1 }
    }
    None
}

/// Where in `s` the emphasis opened by `n` of `mark` closes. Runs between
/// that open and close spans of their own are stepped over, so the `***` at
/// the end of `**a *b***` closes the inner span with its first star and the
/// outer one with the other two.
fn find_close(s: &str, mark: char, n: usize) -> Option<usize> {
    // The lengths of the runs opened inside and not yet closed.
    let mut open: Vec<usize> = vec![];
    let mut at = 0;
    while let Some(found) = s[at..].find(mark) {
        let start = at + found;
        let len = s[start..].chars().take_while(|c| *c == mark).count();
        at = start + len;
        let can_close = s[..start].chars().next_back().is_some_and(|c| !c.is_whitespace());
        let can_open = s[at..].chars().next().is_some_and(|c| !c.is_whitespace());
        let (mut left, mut pos) = (len, start);
        if can_close {
            while let Some(&inner) = open.last().filter(|inner| **inner <= left) {
                open.pop();
                left -= inner;
                pos += inner;
            }
            if open.is_empty() && left == n { return Some(pos) }
        }
        if left > 0 && can_open { open.push(left) }
    }
    None
}

/// `[label](url "title")` at the start of `s`: (label, url, bytes used).
fn link_at(s: &str) -> Option<(&str, &str, usize)> {
    let mut depth = 0;
    let mut close = None;
    for (at, byte) in s.bytes().enumerate() {
        match byte {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(at);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let inner = s[close + 1..].strip_prefix('(')?;
    let end = inner.find(')')?;
    let url = inner[..end].split_whitespace().next().unwrap_or("").trim_matches(['<', '>']);
    link_ok(url).then_some((&s[1..close], url, close + 2 + end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bold() -> Marks {
        Marks { bold: true, ..Marks::default() }
    }

    #[test]
    fn inline_marks() {
        let rich = inline("a **b** `c*d` ~~e~~ *f*");
        assert_eq!(rich.text, "a b c*d e f");
        let marks: Vec<(&str, &Marks)> = rich.spans().map(|(range, marks)| (&rich.text[range], marks)).collect();
        assert_eq!(marks[1], ("b", &bold()));
        assert_eq!(marks[3], ("c*d", &Marks { code: true, ..Marks::default() }));
        assert_eq!(marks[5], ("e", &Marks { strike: true, ..Marks::default() }));
        assert_eq!(marks[7], ("f", &Marks { italic: true, ..Marks::default() }));
        // A span inside a span, both closed by one run of stars.
        let nested = inline("**寄生植物 *Phtheirospermum japonicum*** 和 *a **b** c*");
        assert_eq!(nested.text, "寄生植物 Phtheirospermum japonicum 和 a b c");
        assert_eq!(nested.runs[0], ("寄生植物 ".len(), bold()));
        assert_eq!(nested.runs[1], ("Phtheirospermum japonicum".len(), Marks { bold: true, italic: true, ..Marks::default() }));
        assert_eq!(nested.runs[4].1, Marks { bold: true, italic: true, ..Marks::default() });
        // Chinese text right against the stars, as models write it.
        assert_eq!(inline("扩展到**多种粮食作物**，以及").runs[1], ("多种粮食作物".len(), bold()));
    }

    #[test]
    fn what_does_not_close_stays_as_written() {
        // An answer is drawn while it streams.
        assert_eq!(inline("扩展到**多种粮食").text, "扩展到**多种粮食");
        assert_eq!(inline("a `b").text, "a `b");
        assert_eq!(inline("2 * 3 * 4 and snake_case_name, ~5000 cells").runs.len(), 1);
    }

    #[test]
    fn citations_are_not_links() {
        let rich = inline("见 [P3 p.5 \"a sentence\"] 和 [P148][P296]，以及 [原文](https://doi.org/10.1/x) https://example.org/a.");
        assert!(rich.text.starts_with("见 [P3 p.5 \"a sentence\"] 和 [P148][P296]，以及 原文 https://example.org/a."));
        let links: Vec<(&str, &str)> = rich.spans().filter_map(|(range, marks)| marks.link.as_deref().map(|url| (&rich.text[range], url))).collect();
        assert_eq!(links, [("原文", "https://doi.org/10.1/x"), ("https://example.org/a", "https://example.org/a")]);
    }

    #[test]
    fn blocks_of_an_answer() {
        let parsed = parse("## 覆盖范围\n\n第一行\n第二行\n\n- **根**：拟南芥\n  - 侧根\n- 叶\n\n1. 一\n1. 二\n\n---\n> 引文\n\n```python\nprint(1)\n```\n- [x] 完成");
        assert_eq!(parsed[0], Block::Heading(2, Rich::plain("覆盖范围")));
        assert_eq!(parsed[1], Block::Paragraph(Rich::plain("第一行\n第二行")));
        let Block::Item { marker: Marker::Bullet, text, children } = &parsed[2] else { panic!("{:?}", parsed[2]) };
        assert_eq!((text.text.as_str(), &text.runs[0].1), ("根：拟南芥", &bold()));
        assert_eq!(children[..], [Block::Item { marker: Marker::Bullet, text: Rich::plain("侧根"), children: vec![] }]);
        assert!(matches!(&parsed[4], Block::Item { marker: Marker::Number(1), .. }));
        assert!(matches!(&parsed[5], Block::Item { marker: Marker::Number(2), .. }));
        assert_eq!(parsed[6], Block::Rule);
        assert_eq!(parsed[7], Block::Quote(vec![Block::Paragraph(Rich::plain("引文"))]));
        assert_eq!(parsed[8], Block::Code { lang: "python".into(), text: "print(1)".into() });
        assert!(matches!(&parsed[9], Block::Item { marker: Marker::Todo(true), .. }));
        assert_eq!(parsed.len(), 10);
    }

    #[test]
    fn pipe_tables() {
        let parsed = parse("| 比较维度 | scRNA-seq | snRNA-seq |\n|---|:---:|---:|\n| **RNA 信息范围** | 整细胞 [P260] | 核内 |\n| 材料 | 需酶解 |\n后面的话");
        let Block::Table { aligns, rows } = &parsed[0] else { panic!("{:?}", parsed[0]) };
        assert_eq!(aligns[..], [Align::Left, Align::Center, Align::Right]);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1][0], Rich { text: "RNA 信息范围".into(), runs: vec![("RNA 信息范围".len(), bold())] });
        assert_eq!(rows[1][1].text, "整细胞 [P260]");
        // A short row is filled up.
        assert_eq!(rows[2][2], Rich::default());
        assert_eq!(parsed[1], Block::Paragraph(Rich::plain("后面的话")));
        // Before its second line has arrived, a table is still a paragraph.
        assert!(matches!(parse("| a | b |")[0], Block::Paragraph(_)));
    }
}
