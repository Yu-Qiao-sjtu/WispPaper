//! Page text of a stored full-text file, and the search over it that 文献库提问
//! uses to pick passages.

use std::path::Path;

/// What one paper's text may occupy; a longer text is cut and says so.
pub const MAX_TEXT_BYTES: usize = 8 << 20;

/// The text of a file, page by page. A plain-text or Markdown file is one
/// page. A PDF that cannot be read is an error, never an empty text: an
/// answer should not silently rest on less than the reader thinks.
pub fn file_pages(path: &Path) -> Result<Vec<String>, String> {
    let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let pages = match extension.as_str() {
        "txt" | "md" | "markdown" => vec![std::fs::read_to_string(path).map_err(|e| format!("无法读取文件：{e}"))?],
        "pdf" => {
            let bytes = std::fs::read(path).map_err(|e| format!("无法读取文件：{e}"))?;
            // pdf-extract panics on some malformed files.
            let pages = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem_by_pages(&bytes)).map_err(|_| "PDF 结构异常，无法提取文字。".to_string())?.map_err(|e| format!("无法提取 PDF 文字：{e}"))?;
            pages.into_iter().map(|p| tidy(&p)).collect()
        }
        other => return Err(format!("不支持提取 .{other} 文件的文字。")),
    };
    if pages.iter().all(|p| p.trim().is_empty()) {
        return Err("文件里没有可提取的文字（可能是扫描件）。".into());
    }
    let mut total = 0;
    let mut kept = vec![];
    for page in pages {
        total += page.len();
        if total > MAX_TEXT_BYTES {
            kept.push("[后续页面超出长度上限，未提取]".to_string());
            break;
        }
        kept.push(page);
    }
    Ok(kept)
}

/// Collapse the spacing PDF extraction leaves behind; keep paragraph breaks.
fn tidy(page: &str) -> String {
    let mut out = String::with_capacity(page.len());
    let mut blank = 0;
    for line in page.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() {
            blank += 1;
            continue;
        }
        if !out.is_empty() { out.push_str(if blank > 0 { "\n\n" } else { "\n" }) }
        blank = 0;
        out.push_str(&line);
    }
    out
}

/// A text reduced to what a quotation is compared by: its letters and
/// digits, lowercase, with ligatures spelled out. Spacing, line breaks,
/// hyphens and punctuation differ between a PDF's text and a quotation of
/// it; these do not.
pub fn bare(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            'ﬀ' => out.push_str("ff"),
            'ﬁ' => out.push_str("fi"),
            'ﬂ' => out.push_str("fl"),
            'ﬃ' => out.push_str("ffi"),
            'ﬄ' => out.push_str("ffl"),
            c if c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ => {}
        }
    }
    out
}

/// Where a quotation stands among the glyphs of a page, given the text of
/// each glyph in drawing order: the range of glyphs it covers. When the whole
/// quotation is not there, its first 80, 40 or 20 characters are looked for,
/// so a quotation that drifts from the page after a while is still found.
pub fn find_quote(glyphs: &[String], quote: &str) -> Option<std::ops::Range<usize>> {
    // Every compared character, with the glyph it came from.
    let mut stream: Vec<(char, usize)> = vec![];
    for (index, glyph) in glyphs.iter().enumerate() {
        stream.extend(bare(glyph).chars().map(|c| (c, index)));
    }
    let wanted: Vec<char> = bare(quote).chars().collect();
    for length in [wanted.len(), 80, 40, 20] {
        if length > wanted.len() || length < 8 { continue }
        let part = &wanted[..length];
        if let Some(at) = stream.windows(length).position(|window| window.iter().map(|(c, _)| c).eq(part.iter())) {
            return Some(stream[at].1..stream[at + length - 1].1 + 1);
        }
    }
    None
}

/// Glyph boxes (`[left, top, right, bottom]`, in drawing order) merged into
/// one box per line of text, for highlighting a passage.
pub fn line_boxes(glyphs: &[[f32; 4]]) -> Vec<[f32; 4]> {
    let mut lines: Vec<[f32; 4]> = vec![];
    for glyph in glyphs {
        let same_line = lines.last().is_some_and(|line| {
            let overlap = line[3].min(glyph[3]) - line[1].max(glyph[1]);
            // On the line when it shares most of its height, and does not start far back to the left.
            overlap > 0.5 * (glyph[3] - glyph[1]).min(line[3] - line[1]) && glyph[0] >= line[0] - (glyph[3] - glyph[1])
        });
        match lines.last_mut() {
            Some(line) if same_line => *line = [line[0].min(glyph[0]), line[1].min(glyph[1]), line[2].max(glyph[2]), line[3].max(glyph[3])],
            _ => lines.push(*glyph),
        }
    }
    lines
}

/// The words of a question worth matching: lowercase runs of letters and
/// digits at least two characters long; every CJK character pair counts as a
/// word, since Chinese has no spaces.
pub fn terms(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut push = |word: String| {
        if !out.contains(&word) { out.push(word) }
    };
    let lower = text.to_lowercase();
    let mut word = String::new();
    let mut cjk: Vec<char> = vec![];
    let flush_word = |word: &mut String, push: &mut dyn FnMut(String)| {
        if word.chars().count() >= 2 && !STOP_WORDS.contains(&word.as_str()) { push(std::mem::take(word)) } else { word.clear() }
    };
    for c in lower.chars().chain(std::iter::once(' ')) {
        let is_cjk = ('\u{4e00}'..='\u{9fff}').contains(&c);
        if is_cjk {
            flush_word(&mut word, &mut push);
            cjk.push(c);
            if cjk.len() >= 2 { push(cjk[cjk.len() - 2..].iter().collect()) }
        } else {
            cjk.clear();
            if c.is_alphanumeric() { word.push(c) } else { flush_word(&mut word, &mut push) }
        }
    }
    out
}

const STOP_WORDS: [&str; 44] = [
    "the", "and", "for", "are", "was", "were", "with", "that", "this", "from", "have", "has", "what", "which", "how", "why", "who", "does", "did", "can", "could", "about", "into",
    "their", "there", "these", "those", "any", "all", "not", "but", "its", "our", "you", "your", "they", "them", "than", "then", "been", "being", "of", "in", "on",
];

/// How well a text answers the terms: each term counts once, plus a little for repeats.
pub fn score(text_lower: &str, terms: &[String]) -> f32 {
    let mut total = 0.;
    for term in terms {
        let hits = text_lower.matches(term.as_str()).count();
        if hits > 0 { total += 1. + (hits as f32).ln() * 0.3 }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_of_mixed_text() {
        assert_eq!(terms("What is the role of WOX5 in root 干细胞?"), ["is", "role", "wox5", "root", "干细", "细胞"]);
        assert!(terms("的 a").is_empty());
    }

    #[test]
    fn scoring() {
        let t = terms("root stem cells");
        assert!(score("the root of root stem cells", &t) > score("stem only", &t));
        assert_eq!(score("nothing here", &t), 0.);
    }

    #[test]
    fn text_files_and_errors() {
        let dir = std::env::temp_dir().join(format!("wisp-fulltext-{}", crate::model::new_id("T")));
        std::fs::create_dir_all(&dir).unwrap();
        let txt = dir.join("a.txt");
        std::fs::write(&txt, "hello\nworld").unwrap();
        assert_eq!(file_pages(&txt).unwrap(), ["hello\nworld"]);
        let bad = dir.join("bad.pdf");
        std::fs::write(&bad, "not a pdf").unwrap();
        assert!(file_pages(&bad).is_err());
        assert!(file_pages(&dir.join("x.docx")).is_err());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn quotations_among_glyphs() {
        assert_eq!(bare("Single-cell\nRNA-seq: ﬁve ﬂoors, 10×!"), "singlecellrnaseqfivefloors10");
        // One glyph per character, as a PDF draws them; the ligature is one glyph.
        let glyphs: Vec<String> = "Libraries were prepared with 10x Genomics Chromium. The ﬁrst run failed.".chars().map(String::from).collect();
        let found = find_quote(&glyphs, "prepared with\n10X genomics chromium").unwrap();
        assert_eq!(glyphs[found.clone()].concat(), "prepared with 10x Genomics Chromium");
        assert_eq!(glyphs[find_quote(&glyphs, "The first run").unwrap()].concat(), "The ﬁrst run");
        // A quotation that leaves the page after its beginning is found by that beginning: its first 40 characters here.
        let drifting = find_quote(&glyphs, "Libraries were prepared with 10x Genomics Chromium and then something the page never says at all").unwrap();
        assert_eq!(glyphs[drifting].concat(), "Libraries were prepared with 10x Genomics Chro");
        assert_eq!(find_quote(&glyphs, "nothing of the kind here"), None);
        assert_eq!(find_quote(&glyphs, "with"), None);
        assert_eq!(find_quote(&[], "prepared with 10x"), None);
    }

    #[test]
    fn boxes_by_line() {
        // Two glyphs on one line, a superscript on it, then the next line starting back at the left.
        let glyphs = [[10., 100., 16., 112.], [16., 100., 22., 112.], [22., 97., 25., 106.], [10., 114., 16., 126.], [16., 114., 30., 126.]];
        assert_eq!(line_boxes(&glyphs), [[10., 97., 25., 112.], [10., 114., 30., 126.]]);
        assert!(line_boxes(&[]).is_empty());
    }

    #[test]
    fn tidy_keeps_paragraphs() {
        assert_eq!(tidy("  a   b \n c\n\n\n d "), "a b\nc\n\nd");
    }
}
