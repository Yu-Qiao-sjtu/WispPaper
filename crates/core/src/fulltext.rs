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
    fn tidy_keeps_paragraphs() {
        assert_eq!(tidy("  a   b \n c\n\n\n d "), "a b\nc\n\nd");
    }
}
