//! A paper's full text put in order for writing: its pages cut into
//! paragraphs and headings (`blocks`), every paragraph given the part of the
//! paper it belongs to (`assign` by rule, or a model's reading of the
//! outline: `outline_prompt` / `parse_outline`), the paragraphs cut into
//! sentences (`sentences`), and the sentences of one part ranked against
//! what a person is writing (`rank`). 写作 shows those as candidates: write
//! an introduction, read how the library's introductions put it.

use crate::fulltext::bare;
use std::collections::{HashMap, HashSet};

/// The parts a paper is cut into, as stored and as shown. `other` is body
/// text whose part is not known; what is left out (front matter, captions,
/// references) has no key here and is not kept.
pub const SECTIONS: [(&str, &str); 6] = [("abstract", "摘要"), ("introduction", "导言"), ("methods", "方法"), ("results", "结果"), ("discussion", "讨论"), ("other", "其他")];

/// The label of a block that is not kept.
pub const SKIP: &str = "skip";

/// Bumped when `blocks`, `assign` or `sentences` change what they make of
/// the same pages, so structures made by rule are made again.
pub const VERSION: i32 = 1;

/// A paragraph or a heading of a paper, in reading order.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// The 1-based page it starts on.
    pub page: usize,
    pub text: String,
    /// A short line standing alone: a section title, or something that looks like one.
    pub heading: bool,
}

/// A sentence of a paper, with where it stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sentence {
    /// A key of `SECTIONS`.
    pub section: String,
    /// The heading it stands under; empty when there is none.
    pub heading: String,
    pub page: usize,
    /// Its place among the sentences of this part of the paper: 0 opens the part.
    pub position: usize,
    pub text: String,
}

/// A line with its digits blanked: what a running header looks like from page to page.
fn shape(line: &str) -> String {
    line.chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect::<String>().to_lowercase()
}

/// The lines that come back on page after page: running headers and
/// footers, journal names, download notices.
fn furniture(pages: &[String]) -> HashSet<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for page in pages {
        let shapes: HashSet<String> = page.lines().map(str::trim).filter(|l| !l.is_empty() && l.chars().count() <= 120).map(shape).collect();
        for shape in shapes {
            *seen.entry(shape).or_default() += 1;
        }
    }
    let enough = (pages.len() / 4).max(3);
    seen.into_iter().filter(|(_, count)| *count >= enough).map(|(shape, _)| shape).collect()
}

/// The words the text writes with a hyphen inside a line, so a hyphen at
/// a line's end can be told from one that only breaks the word.
fn hyphenated(pages: &[String]) -> HashSet<String> {
    let mut words = HashSet::new();
    for page in pages {
        for word in page.split_whitespace() {
            let word = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
            if word.contains('-') && word.chars().filter(|c| c.is_alphabetic()).count() >= 4 { words.insert(word); }
        }
    }
    words
}

/// The citation numbers a superscript leaves at the start of the next
/// line (`stresses` / `1,2. Roots have`): how many bytes of the line they take.
fn leading_citation(line: &str) -> usize {
    let mut end = 0;
    let mut digits = 0;
    for (at, c) in line.char_indices() {
        match c {
            '0'..='9' => digits += 1,
            ',' | '-' | '–' if digits > 0 => digits = 0,
            '.' | ',' | ';' | ':' | ')' if digits > 0 => {
                end = at;
                break;
            }
            _ => return 0,
        }
        if digits > 3 { return 0 }
    }
    // Only before a punctuation mark that ends the clause: `3.5 cm` and `47,000 cells` stay.
    let rest = &line[end..];
    if end > 0 && rest.len() > 1 && rest[1..].starts_with(' ') { end } else { 0 }
}

/// Lines of one paragraph joined into running text.
fn join_lines(lines: &[&str], hyphenated: &HashSet<String>) -> String {
    let mut out = String::new();
    for line in lines {
        let mut line = *line;
        let last_letter = out.chars().last().is_some_and(char::is_alphabetic);
        if last_letter {
            let skip = leading_citation(line);
            if skip > 0 {
                line = &line[skip..];
                out.push_str(line);
                continue;
            }
        }
        let next_lower = line.chars().next().is_some_and(char::is_lowercase);
        if out.ends_with('-') && next_lower && out.len() > 1 {
            // `protoplast-` / `ing` is one word; `single-` / `cell` keeps its hyphen when the paper writes it so elsewhere.
            let head: String = out.chars().rev().skip(1).take_while(|c| c.is_alphanumeric() || *c == '-').collect::<Vec<_>>().into_iter().rev().collect();
            let tail: String = line.chars().take_while(|c| c.is_alphanumeric()).collect();
            if !hyphenated.contains(&format!("{head}-{tail}").to_lowercase()) { out.pop(); }
            out.push_str(line);
            continue;
        }
        if !out.is_empty() { out.push(' ') }
        out.push_str(line);
    }
    out
}

/// Whether a block of these lines looks like a title standing alone.
fn looks_like_heading(lines: &[&str], text: &str) -> bool {
    let count = text.chars().count();
    if lines.len() > 2 || !(3..=110).contains(&count) { return false }
    let Some(first) = text.chars().next() else { return false };
    if !(first.is_uppercase() || first.is_ascii_digit()) { return false }
    if text.ends_with(['.', ',', ';', ':', '?']) && canonical(text).is_none() { return false }
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    letters * 10 >= count * 6 && !is_caption(text)
}

/// A figure or table legend.
fn is_caption(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["fig.", "fig ", "figure ", "table ", "extended data", "supplementary fig", "supplemental fig", "scheme "].iter().any(|start| {
        lower.strip_prefix(start).is_some_and(|rest| rest.trim_start().chars().next().is_some_and(|c| c.is_ascii_digit() || c == 's' || c == 'f' || c == 't'))
    })
}

/// The small print of a first or last page: licence, copyright, correspondence, dates.
fn is_notice(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["creative commons", "licensed under", "© the author", "the author(s)", "all rights reserved", "correspondence:", "*correspondence", "received:", "accepted:", "published online", "publisher's note", "publisher’s note", "doi.org/", "open access this article", "these authors contributed equally", "handling editor", "edited by", "reviewed by", "peer review"]
        .iter()
        .any(|mark| lower.contains(mark))
}

/// The names of parts that some layouts run into the paragraph they head
/// (`Introduction Single-cell RNA sequencing has…`), longest first.
const RUN_IN: [&str; 23] = [
    "Materials and methods", "Materials and Methods", "MATERIALS AND METHODS", "Results and discussion", "Results and Discussion", "Introduction", "INTRODUCTION", "Background", "BACKGROUND", "Conclusions", "CONCLUSIONS", "Conclusion", "CONCLUSION", "Discussion", "DISCUSSION",
    "References", "REFERENCES", "Abstract", "ABSTRACT", "Results", "RESULTS", "Methods", "METHODS",
];

/// A paragraph that opens with the name of a part and goes straight on
/// with a new sentence: the name, and the paragraph without it.
fn run_in(text: &str) -> Option<(&'static str, &str)> {
    let name = RUN_IN.iter().find(|name| text.starts_with(**name))?;
    let rest = text[name.len()..].strip_prefix(' ')?.trim_start();
    (rest.len() >= 40 && rest.chars().next().is_some_and(char::is_uppercase)).then_some((*name, rest))
}

/// Running text: long enough, and it says something to its end.
fn is_prose(block: &Block) -> bool {
    !block.heading && block.text.chars().count() >= 200 && block.text.contains(". ")
}

/// The paragraphs and headings of a paper's pages, in reading order, with
/// running headers left out, the words broken at line ends put together
/// and a paragraph that runs over a page joined again.
pub fn blocks(pages: &[String]) -> Vec<Block> {
    let furniture = furniture(pages);
    let hyphenated = hyphenated(pages);
    let mut out: Vec<Block> = vec![];
    for (index, page) in pages.iter().enumerate() {
        let mut lines: Vec<&str> = vec![];
        for line in page.lines().chain([""]) {
            let line = line.trim();
            let is_number = !line.is_empty() && line.chars().all(|c| c.is_ascii_digit());
            if !line.is_empty() && !is_number && !furniture.contains(&shape(line)) {
                lines.push(line);
                continue;
            }
            // A header in the middle of a paragraph does not end it; a blank line does.
            if !line.is_empty() || lines.is_empty() { continue }
            let text = join_lines(&lines, &hyphenated);
            let heading = looks_like_heading(&lines, &text);
            lines.clear();
            // What starts in small letters carries on the paragraph left open before the page or column ended.
            let continues = !heading && text.chars().next().is_some_and(char::is_lowercase);
            let open = out.iter().rposition(|b| is_prose(b) || (!b.heading && b.text.chars().count() >= 80)).filter(|at| out.len() - at <= 12 && !out[*at].text.ends_with(['.', '!', '?', ':', '”', '"', ')']));
            match open.filter(|_| continues) {
                Some(at) => {
                    let block = &mut out[at];
                    if block.text.ends_with('-') {
                        block.text.pop();
                    } else {
                        block.text.push(' ');
                    }
                    block.text.push_str(&text);
                }
                None => match run_in(&text).filter(|_| !heading) {
                    Some((name, rest)) => out.extend([Block { page: index + 1, text: name.into(), heading: true }, Block { page: index + 1, text: rest.into(), heading: false }]),
                    None => out.push(Block { page: index + 1, text, heading }),
                },
            }
        }
    }
    out
}

/// The part of a paper a heading names: a key of `SECTIONS`, or `SKIP` for
/// the back matter. `None` for a heading that is the paper's own.
pub fn canonical(heading: &str) -> Option<&'static str> {
    // `2.1 Materials and Methods`, `II. RESULTS`, `R E S U L T S`, `STAR★METHODS`.
    let mut name: String = heading.replace('ﬁ', "fi").replace('ﬂ', "fl").to_lowercase().chars().map(|c| if c.is_alphabetic() || c == '&' { c } else { ' ' }).collect();
    if name.split_whitespace().count() >= 4 && name.split_whitespace().all(|word| word.chars().count() == 1) { name = name.replace(' ', "") }
    let words: Vec<&str> = name.split_whitespace().skip_while(|word| ["i", "ii", "iii", "iv", "v", "vi"].contains(word)).collect();
    let name = words.join(" ").replace(" & ", " and ");
    const NAMES: [(&str, &[&str]); 6] = [
        ("abstract", &["abstract", "summary", "in brief", "highlights"]),
        ("introduction", &["introduction", "background", "main"]),
        ("methods", &["methods", "method", "materials and methods", "material and methods", "methods and materials", "experimental procedures", "experimental section", "online methods", "star methods", "method details", "methodology", "materials and method", "experimental model and subject details", "quantification and statistical analysis"]),
        ("results", &["results", "result", "results and discussion", "results and discussions", "findings"]),
        ("discussion", &["discussion", "discussions", "conclusion", "conclusions", "concluding remarks", "discussion and conclusion", "discussion and conclusions", "conclusions and perspectives", "conclusion and perspectives", "conclusions and future perspectives", "conclusion and future perspectives", "summary and outlook", "perspectives", "outlook", "limitations of the study"]),
        (SKIP, &[
            "references", "reference", "literature cited", "bibliography", "acknowledgements", "acknowledgments", "acknowledgement", "acknowledgment", "funding", "author contributions", "authors contributions", "contributions", "data availability",
            "data availability statement", "code availability", "data and code availability", "availability of data and materials", "competing interests", "conflict of interest", "conflicts of interest", "declaration of interests", "declarations", "ethics declarations",
            "supplementary information", "supplemental information", "supplementary material", "supplementary materials", "supporting information", "supplementary data", "additional information", "abbreviations", "reporting summary", "online content", "accession numbers",
            "key resources table", "resource availability", "author information", "publisher s note", "keywords", "key words", "generative ai statement",
        ]),
    ];
    NAMES.iter().find(|(_, names)| names.contains(&name.as_str())).map(|(key, _)| *key)
}

/// The part of the paper each block belongs to, told by rule: a heading
/// that names a part starts it. In a paper that never says "Introduction"
/// (Nature's way), the running text before the first heading of its own is
/// the introduction, and what follows it the results. `abstract_text` is
/// the abstract of the paper's record: the paragraph that repeats it is the
/// abstract. Front matter, legends and back matter are `SKIP`.
pub fn assign(blocks: &[Block], abstract_text: &str) -> Vec<&'static str> {
    assign_sure(blocks, abstract_text).0
}

/// `assign`, and for each block whether the rules are sure of it: it
/// stands under a heading that names its part, or is plainly not body
/// text. The rest they only guess at (text before any such heading, a
/// paper's own headings in a review), and a model may know better.
pub fn assign_sure(blocks: &[Block], abstract_text: &str) -> (Vec<&'static str>, Vec<bool>) {
    let abstract_start: String = bare(abstract_text).chars().take(60).collect();
    let named_introduction = blocks.iter().any(|b| b.heading && canonical(&b.text) == Some("introduction"));
    let named_results = blocks.iter().any(|b| b.heading && canonical(&b.text) == Some("results"));
    // A review has neither methods nor results: its own headings are its body.
    let review = !named_results && !blocks.iter().any(|b| b.heading && canonical(&b.text) == Some("methods"));
    // Before the first part: front matter, unless the paper has no "Introduction" to wait for.
    let mut section = SKIP;
    let (mut implicit, mut guessed) = (false, true);
    let (mut labels, mut sure) = (vec![], vec![]);
    for block in blocks {
        if block.heading {
            match canonical(&block.text) {
                Some(part) => (section, implicit, guessed) = (part, false, false),
                None if section == "introduction" && review => (section, guessed) = ("other", true),
                // The first heading of the paper's own after an unnamed introduction opens the results.
                None if implicit && section == "introduction" && !named_results => section = "results",
                None => {}
            }
            labels.push(SKIP);
            sure.push(true);
            continue;
        }
        if is_caption(&block.text) || is_notice(&block.text) {
            labels.push(SKIP);
            sure.push(true);
            continue;
        }
        if abstract_start.chars().count() >= 40 && bare(&block.text).contains(&abstract_start) {
            labels.push("abstract");
            sure.push(true);
            continue;
        }
        if section == SKIP && !named_introduction && !implicit && is_prose(block) && labels.iter().all(|l| *l == SKIP || *l == "abstract") && !block.text.contains('@') {
            (section, implicit) = ("introduction", true);
        }
        labels.push(if section == "abstract" && !is_prose(block) { SKIP } else { section });
        sure.push(!guessed);
    }
    (labels, sure)
}

/// How many paragraphs of running text the rules only guessed at: with a
/// few, a model's reading of the outline is worth asking for.
pub fn open_paragraphs(blocks: &[Block], sure: &[bool]) -> usize {
    blocks.iter().zip(sure).filter(|(block, sure)| !**sure && is_prose(block)).count()
}

/// From this many guessed paragraphs on, a full text is one for a model to
/// look at: fewer are the front matter of a paper whose parts are named.
pub const OPEN_ENOUGH: usize = 4;

/// The rules' labels where they are sure, the model's elsewhere.
pub fn merge(rules: &[&'static str], sure: &[bool], model: &[&'static str]) -> Vec<&'static str> {
    rules.iter().zip(sure).zip(model).map(|((rule, sure), model)| if *sure { *rule } else { *model }).collect()
}

/// What a model is told when it is asked to sort a paper's blocks into parts.
pub const OUTLINE_SYSTEM: &str = "你在整理一篇学术论文的版面提取结果。提取结果按阅读顺序分成了编号的块：标题行（H）和正文段落（P），每块只给出开头。你的任务是判断每一块属于论文的哪个部分。\n\
部分只有这几种：abstract（摘要）、introduction（导言，含背景）、methods（材料与方法）、results（结果）、discussion（讨论，含结论）、other（属于正文但不属于以上任何一种，例如综述的各个主题章节）。\n\
不属于正文的块不要列出：题目、作者、单位、通讯信息、收稿日期、页眉页脚、图注和表注、表格内容、致谢、基金、作者贡献、数据可用性、参考文献、补充材料说明。\n\
注意：摘要常常没有标题，是题目和作者之后、正文之前概述全文的那一段，它是 abstract 而不是 introduction。有的期刊（如 Nature）导言没有标题，直接跟在摘要后面；结果部分常由论文自己的小标题组成；方法可能排在参考文献之后。小标题下的段落归入它所在的部分。\n\
只输出结果，每行一个部分，写出属于它的块编号范围，例如：\nintroduction: 6-11\nresults: 12-58, 61-64\nmethods: 80-112\n不要解释。";

/// How much of a block the model sees.
const OUTLINE_CHARS: usize = 110;
/// The blocks one request lists; a longer paper is cut there and says so.
pub const OUTLINE_BLOCKS: usize = 900;

/// The blocks as a numbered outline for the model: `12 p3 H Results`.
pub fn outline_prompt(title: &str, blocks: &[Block]) -> String {
    let mut out = format!("论文题目：{title}\n\n");
    for (index, block) in blocks.iter().take(OUTLINE_BLOCKS).enumerate() {
        let count = block.text.chars().count();
        let start: String = block.text.chars().take(OUTLINE_CHARS).collect();
        out.push_str(&format!("{index} p{} {} {start}{}\n", block.page, if block.heading { "H" } else { "P" }, if count > OUTLINE_CHARS { format!("…（{count} 字符）") } else { String::new() }));
    }
    if blocks.len() > OUTLINE_BLOCKS { out.push_str("（后面的块没有列出）\n") }
    out
}

/// The model's answer to `outline_prompt`, as a label for each of the
/// `count` blocks. A block it did not list is left out; headings never
/// carry sentences. An answer that names no block at all is an error.
pub fn parse_outline(reply: &str, blocks: &[Block], abstract_text: &str) -> Result<Vec<&'static str>, String> {
    let abstract_start: String = bare(abstract_text).chars().take(60).collect();
    let mut labels = vec![SKIP; blocks.len()];
    let mut named = 0;
    for line in reply.split(['\n', ';', '；']) {
        let line = line.trim().trim_start_matches(['-', '*', ' ']).to_lowercase();
        let Some((key, _)) = SECTIONS.iter().find(|(key, name)| line.starts_with(key) || line.starts_with(name)) else { continue };
        let ranges = line.split_once([':', '：']).map_or("", |(_, ranges)| ranges);
        for range in ranges.split([',', '，', '、']) {
            let mut ends = range.split(['-', '–', '~', '—']).map(|n| n.trim().parse::<usize>());
            let (Some(Ok(from)), to) = (ends.next(), ends.next()) else { continue };
            let to = match to {
                Some(Ok(to)) => to,
                Some(Err(_)) => continue,
                None => from,
            };
            for index in from..=to.min(blocks.len().saturating_sub(1)) {
                if index < blocks.len() && !blocks[index].heading {
                    labels[index] = key;
                    named += 1;
                }
            }
        }
    }
    if named == 0 { return Err("模型的回答里没有可用的分节结果。".into()) }
    // A legend stays out whatever part its neighbours are in, and the abstract is the one the record has.
    for (label, block) in labels.iter_mut().zip(blocks) {
        if is_caption(&block.text) { *label = SKIP }
        if *label != SKIP && abstract_start.chars().count() >= 40 && bare(&block.text).contains(&abstract_start) { *label = "abstract" }
    }
    Ok(labels)
}

const ABBREVIATIONS: [&str; 30] = [
    "al", "fig", "figs", "e.g", "i.e", "vs", "cf", "ca", "approx", "ref", "refs", "eq", "eqs", "no", "nos", "dr", "prof", "sp", "spp", "var", "supp", "suppl", "tab", "st", "inc", "ltd", "co", "ed", "eds", "vol",
];

/// A text cut into sentences. A full stop after an abbreviation, an
/// initial or inside a number does not end one.
pub fn split_sentences(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = vec![];
    let mut start = 0;
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if matches!(c, '.' | '!' | '?') {
            // Closing marks belong to the sentence they close.
            let mut end = at + 1;
            while end < chars.len() && matches!(chars[end], ')' | ']' | '"' | '”' | '’' | '\'') { end += 1 }
            let after_space = end < chars.len() && chars[end].is_whitespace();
            let next = chars[end..].iter().copied().find(|c| !c.is_whitespace());
            let starts_new = next.is_some_and(|n| n.is_uppercase() || n.is_ascii_digit() || matches!(n, '(' | '[' | '“' | '"'));
            let word: String = chars[start..at].iter().rev().take_while(|c| !c.is_whitespace() && **c != '(').collect::<Vec<_>>().into_iter().rev().collect();
            let word = word.to_lowercase();
            let abbreviation = c == '.' && (ABBREVIATIONS.contains(&word.trim_start_matches(['[', '“', '"'])) || (word.chars().count() == 1 && word.chars().all(char::is_alphabetic)) || word.ends_with("et al"));
            if (end == chars.len() || (after_space && starts_new)) && !abbreviation {
                let sentence: String = chars[start..end].iter().collect();
                let sentence = sentence.split_whitespace().collect::<Vec<_>>().join(" ");
                if !sentence.is_empty() { out.push(sentence) }
                start = end;
                at = end;
                continue;
            }
        }
        at += 1;
    }
    let rest: String = chars[start..].iter().collect();
    let rest = rest.split_whitespace().collect::<Vec<_>>().join(" ");
    if !rest.is_empty() { out.push(rest) }
    out
}

/// Whether a sentence is one to show a writer: whole, in words, of a
/// length a sentence has. Table rows, addresses and fragments are not.
pub fn usable(sentence: &str) -> bool {
    let count = sentence.chars().count();
    if !(40..=600).contains(&count) || sentence.split_whitespace().count() < 6 { return false }
    let letters = sentence.chars().filter(|c| c.is_alphabetic()).count();
    let first = sentence.chars().next().unwrap_or(' ');
    letters * 10 >= count * 7 && (first.is_uppercase() || first.is_ascii_digit()) && sentence.ends_with(['.', '?', '!', ')', '”', '"']) && !sentence.contains('@')
}

/// The sentences of the blocks that are kept, with their part, the
/// heading they stand under and their place in the part. The abstract is
/// not among them: a paper's record has it.
pub fn sentences(blocks: &[Block], labels: &[&str]) -> Vec<Sentence> {
    let mut out = vec![];
    let mut heading = String::new();
    let mut positions: HashMap<&str, usize> = HashMap::new();
    for (block, label) in blocks.iter().zip(labels) {
        if block.heading {
            heading = if canonical(&block.text).is_some() { String::new() } else { block.text.clone() };
            continue;
        }
        if *label == SKIP || *label == "abstract" { continue }
        for text in split_sentences(&block.text).into_iter().filter(|s| usable(s)) {
            let position = positions.entry(label).or_default();
            out.push(Sentence { section: label.to_string(), heading: heading.clone(), page: block.page, position: *position, text });
            *position += 1;
        }
    }
    out
}

/// The sentences of a paper's pages, sorted into parts by rule, and how
/// many paragraphs the rules only guessed at (`open_paragraphs`).
pub fn by_rule(pages: &[String], abstract_text: &str) -> (Vec<Sentence>, usize) {
    let blocks = blocks(pages);
    let (labels, sure) = assign_sure(&blocks, abstract_text);
    (sentences(&blocks, &labels), open_paragraphs(&blocks, &sure))
}

/// What a person is writing right now, for finding sentences like it:
/// the paragraph the caret is in, up to the caret; when the caret stands
/// at the paragraph's start, the paragraph's beginning.
pub fn writing_at(text: &str, caret: usize) -> String {
    let caret = text.floor_char_boundary(caret.min(text.len()));
    let start = text[..caret].rfind('\n').map_or(0, |at| at + 1);
    let end = text[caret..].find('\n').map_or(text.len(), |at| caret + at);
    let written = text[start..caret].trim();
    if written.is_empty() {
        text[start..end].trim().chars().take(WRITING_CHARS).collect()
    } else {
        // The last sentences count: the start of a long paragraph was written a while ago.
        let skip = written.chars().count().saturating_sub(WRITING_CHARS);
        written.chars().skip(skip).collect()
    }
}

const WRITING_CHARS: usize = 320;

/// Whether a text is (partly) in Chinese: the library's sentences are not,
/// so it is put into English search words before it is looked for.
pub fn in_chinese(text: &str) -> bool {
    text.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
}

/// What a model is told to turn something written in Chinese into search words.
pub const KEYWORDS_SYSTEM: &str = "用户正在写英文学术论文，下面是他想表达的意思（中文或中英混杂）。把它改写成用来检索相似英文句子的关键词：给出 6 到 12 个英文单词或短语，覆盖其中的概念、对象和论述方式（例如 remains unclear、in contrast、we hypothesized）。术语用该领域论文里通行的写法。只输出关键词，用空格分隔，不要解释。";

const STOP_WORDS: [&str; 70] = [
    "the", "and", "for", "are", "was", "were", "with", "that", "this", "from", "have", "has", "had", "which", "what", "how", "why", "who", "does", "did", "can", "could", "about", "into", "their", "there", "these", "those", "any", "all", "not", "but", "its", "our", "you",
    "your", "they", "them", "than", "then", "been", "being", "of", "in", "on", "to", "is", "as", "by", "be", "we", "it", "an", "at", "or", "also", "such", "may", "more", "most", "between", "both", "however", "here", "using", "used", "use", "study", "show", "shown",
];

/// A word reduced to what its forms share: `regulates`, `regulated` and
/// `regulating` are one word to a search.
fn stem(word: &str) -> String {
    let count = word.chars().count();
    if !word.is_ascii() { return word.to_string() }
    for (suffix, least, put) in [("ies", 5, "y"), ("sses", 6, "ss"), ("ions", 7, ""), ("ion", 6, ""), ("ing", 6, ""), ("ed", 5, ""), ("es", 5, ""), ("s", 4, "")] {
        if count >= least && word.ends_with(suffix) && !(suffix == "s" && (word.ends_with("ss") || word.ends_with("us") || word.ends_with("is"))) {
            let mut stem = format!("{}{put}", &word[..word.len() - suffix.len()]);
            // `regulat(e)`: the forms with and without the final e meet.
            if stem.ends_with('e') && stem.len() > 4 { stem.pop(); }
            return stem;
        }
    }
    if count > 4 && word.ends_with('e') { word[..word.len() - 1].to_string() } else { word.to_string() }
}

/// The words a sentence is found by: lowercase, without the commonest
/// ones, each reduced to its stem.
pub fn words(text: &str) -> Vec<String> {
    text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|word| word.chars().count() >= 2 && !STOP_WORDS.contains(word)).map(stem).collect()
}

/// The sentences one paper may place among the candidates: a paper that
/// says it all would otherwise fill the list.
const PER_PAPER: usize = 3;

/// The sentences that say most of what `query` says, the best first:
/// `(index into texts, score)`. Words rare among the sentences count for
/// more, a long sentence gets no credit for its length (BM25). `owners`
/// names the paper of each sentence, so no paper takes more than a few
/// places. An empty query finds nothing.
pub fn rank(texts: &[&str], owners: &[&str], query: &str, limit: usize) -> Vec<(usize, f32)> {
    let wanted: Vec<String> = {
        let mut seen = HashSet::new();
        words(query).into_iter().filter(|word| seen.insert(word.clone())).collect()
    };
    if wanted.is_empty() || texts.is_empty() { return vec![] }
    let bags: Vec<Vec<String>> = texts.iter().map(|text| words(text)).collect();
    let average = bags.iter().map(Vec::len).sum::<usize>() as f32 / bags.len() as f32;
    let mut scored: Vec<(usize, f32)> = vec![];
    let weights: Vec<f32> = wanted
        .iter()
        .map(|word| {
            let holders = bags.iter().filter(|bag| bag.contains(word)).count() as f32;
            (1. + (bags.len() as f32 - holders + 0.5) / (holders + 0.5)).ln()
        })
        .collect();
    for (index, bag) in bags.iter().enumerate() {
        let mut score = 0.;
        for (word, weight) in wanted.iter().zip(&weights) {
            let hits = bag.iter().filter(|w| *w == word).count() as f32;
            if hits > 0. { score += weight * hits * 2.2 / (hits + 1.2 * (0.25 + 0.75 * bag.len() as f32 / average.max(1.))) }
        }
        if score > 0. { scored.push((index, score)) }
    }
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut taken: HashMap<&str, usize> = HashMap::new();
    scored
        .into_iter()
        .filter(|(index, _)| {
            let count = taken.entry(owners.get(*index).copied().unwrap_or("")).or_default();
            *count += 1;
            *count <= PER_PAPER
        })
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(text: &str) -> String {
        text.to_string()
    }

    const INTRO: &str = "Crops such as rice thrive in arable soils that show natural heterogeneity. The heterogeneous nature of soils is characterized by uneven distributions of nutrients, water and organic content. However, our understanding of how roots respond to complex soil environments at a cellular level remains limited.";
    const RESULT: &str = "Protoplasts of rice primary roots were adopted to generate a high-quality reference dataset. Using the 10X Genomics platform, we profiled more than 47,000 root cells across ten sets of seedlings. All datasets underwent processing with the same pipeline (Fig. 1a,b).";

    #[test]
    fn blocks_of_pages() {
        let pages = [
            page("Journal of Roots 12 (2024) 100\n\nArticle\n\nA study of roots\n\nThe root is long and it is known that roots have evolved diverse strate-\ngies to tackle the variability in soil condi-\ntions\n1,2. However, single-\ncell work on the single-cell level is rare and the understanding of\n47,000 cells remains"),
            page("Journal of Roots 12 (2024) 101\n\nlimited to this day.\n\nResults\n\nFig. 1 | A legend of the first figure, long enough.\n\n101"),
            page("Journal of Roots 12 (2024) 102\n\nDiscussion More text here, after a name of a part run into it.\n\nResults from elsewhere stay."),
            page("Journal of Roots 12 (2024) 103\n\nThe end."),
        ];
        let blocks = blocks(&pages);
        let texts: Vec<&str> = blocks.iter().map(|b| b.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Article",
                "A study of roots",
                "The root is long and it is known that roots have evolved diverse strategies to tackle the variability in soil conditions. However, single-cell work on the single-cell level is rare and the understanding of 47,000 cells remains limited to this day.",
                "Results",
                "Fig. 1 | A legend of the first figure, long enough.",
                "Discussion",
                "More text here, after a name of a part run into it.",
                "Results from elsewhere stay.",
                "The end."
            ]
        );
        assert_eq!(blocks.iter().map(|b| b.heading).collect::<Vec<_>>(), [true, true, false, true, false, true, false, false, false]);
        assert_eq!((blocks[2].page, blocks[3].page), (1, 2));
    }

    #[test]
    fn headings_that_name_a_part() {
        for (heading, part) in [("1. Introduction", "introduction"), ("MATERIALS AND METHODS", "methods"), ("2.1 Materials & Methods", "methods"), ("R E S U L T S", "results"), ("STAR★METHODS", "methods"), ("Results and Discussion", "results"), ("IV. CONCLUSIONS", "discussion"), ("References", SKIP), ("Acknowledgements", SKIP)] {
            assert_eq!(canonical(heading), Some(part), "{heading}");
        }
        assert_eq!(canonical("Rice root scRNA-seq and spatial transcriptomic atlas"), None);
        assert_eq!(canonical("Methods for the future of results"), None);
    }

    fn block(text: &str) -> Block {
        Block { page: 1, text: text.into(), heading: !text.contains(". ") && text.len() < 100 }
    }

    #[test]
    fn parts_by_rule() {
        // A paper that names its parts.
        let named = [block("A study"), block(INTRO), block("Introduction"), block(INTRO), block("Results"), block("A heading of its own"), block(RESULT), block("Fig. 1 | Legend. With two sentences."), block("Discussion"), block(INTRO), block("References"), block(INTRO), block("Methods"), block(RESULT)];
        assert_eq!(assign(&named, ""), [SKIP, SKIP, SKIP, "introduction", SKIP, SKIP, "results", SKIP, SKIP, "discussion", SKIP, SKIP, SKIP, "methods"]);
        // One that does not: the abstract is known by the record, the text after it is the introduction until the first heading.
        let summary = "Land plants thrive in soils showing vastly different properties and environmental stresses. Root systems can adapt to contrasting soil conditions, yet how their responses are programmed remains unclear. We showed major expression changes in outer root cell types.";
        let nature = [block("Article"), block(summary), block(INTRO), block(INTRO), block("Rice root atlas"), block(RESULT), block("Discussion"), block(INTRO), block("Methods"), block(RESULT)];
        assert_eq!(assign(&nature, &summary.replace("thrive in", "thrive  in\n")), [SKIP, "abstract", "introduction", "introduction", SKIP, "results", SKIP, "discussion", SKIP, "methods"]);
        // The rules are sure of what stands under a named heading, and guess at the rest.
        let (labels, sure) = assign_sure(&nature, summary);
        assert_eq!(sure, [true, true, false, false, true, false, true, true, true, true]);
        assert_eq!(open_paragraphs(&nature, &sure), 3);
        assert_eq!(merge(&labels, &sure, &["other"; 10]), [SKIP, "abstract", "other", "other", SKIP, "other", SKIP, "discussion", SKIP, "methods"]);
        assert_eq!(open_paragraphs(&named, &assign_sure(&named, "").1), 1);
        let made = sentences(&nature, &assign(&nature, summary));
        assert_eq!(made.iter().filter(|s| s.section == "introduction").count(), 6);
        assert_eq!(made.iter().find(|s| s.section == "results").map(|s| (s.heading.as_str(), s.position)), Some(("Rice root atlas", 0)));
        assert!(made.iter().all(|s| s.section != "abstract"));
        assert_eq!(made.iter().filter(|s| s.section == "methods").map(|s| s.position).collect::<Vec<_>>(), [0, 1, 2]);
    }

    #[test]
    fn parts_by_a_model() {
        let blocks = [block("Title"), block(INTRO), block("Atlas"), block(RESULT), block("Fig. 2 | Legend. Two sentences."), block(RESULT)];
        let prompt = outline_prompt("A study", &blocks);
        assert!(prompt.contains("0 p1 H Title\n") && prompt.contains("1 p1 P Crops such as rice") && prompt.contains("字符）"));
        let labels = parse_outline("好的：\nintroduction: 1\n- Results: 2-9\n方法：\nnonsense: 0", &blocks, "").unwrap();
        assert_eq!(labels, [SKIP, "introduction", SKIP, "results", SKIP, "results"]);
        assert_eq!(parse_outline("导言：1；结果：3", &blocks, "").unwrap()[1], "introduction");
        assert_eq!(parse_outline("introduction: 1", &blocks, INTRO).unwrap()[1], "abstract");
        assert!(parse_outline("I cannot tell.", &blocks, "").is_err());
    }

    #[test]
    fn sentences_of_a_text() {
        let text = "Roots adapt to soil (Smith et al. 2019; Fig. 1a). A. thaliana grows fast, e.g. in 3.5 days! Is it so? \"Yes.\" It was shown by Zhu et al. (2021) that it is.";
        assert_eq!(split_sentences(text), ["Roots adapt to soil (Smith et al. 2019; Fig. 1a).", "A. thaliana grows fast, e.g. in 3.5 days!", "Is it so?", "\"Yes.\"", "It was shown by Zhu et al. (2021) that it is."]);
        assert!(usable("However, our understanding of how roots respond to soil remains limited."));
        for bad in ["Too short.", "1Department of Biology, Duke University, Durham, NC, USA. x@duke.edu is the address.", "however, a fragment that starts in the middle of things goes on.", "A sentence that never comes to an end and so is likely a heading or a row of a table", "12.5 33.1 45.6 78.9 11.2 0.05 0.01 ns ns 14.2 15.5 16.7 18.9."] {
            assert!(!usable(bad), "{bad}");
        }
    }

    #[test]
    fn what_is_being_written() {
        let text = "First paragraph, done.\nRoots adapt to soil. However, how they\n\nThird.";
        assert_eq!(writing_at(text, text.find("\n\n").unwrap()), "Roots adapt to soil. However, how they");
        assert_eq!(writing_at(text, 23), "Roots adapt to soil. However, how they");
        assert_eq!(writing_at(text, 5), "First");
        assert_eq!(writing_at("根系如何", 100), "根系如何");
        assert_eq!(writing_at("", 0), "");
        assert!(in_chinese("root 适应 soil") && !in_chinese("root–soil"));
    }

    #[test]
    fn candidates_by_relevance() {
        let texts = [
            "However, how roots respond to soil compaction at the cellular level remains poorly understood.",
            "Single-cell RNA sequencing has transformed the study of plant development.",
            "Roots respond to compacted soils by regulating cell wall remodelling.",
            "The regulation of root growth by auxin is well known.",
            "We thank the members of the laboratory for discussions.",
        ];
        let owners = ["a", "a", "b", "b", "c"];
        let found = rank(&texts, &owners, "root response to soil compaction", 10);
        // The two that speak of roots in compacted soil, then the one that only speaks of roots.
        let order: Vec<usize> = found.iter().map(|(index, _)| *index).collect();
        assert!(order == [0, 2, 3] || order == [2, 0, 3], "{order:?}");
        let mut regulated: Vec<usize> = rank(&texts, &owners, "regulated", 10).iter().map(|(i, _)| *i).collect();
        regulated.sort();
        assert_eq!(regulated, [2, 3]);
        assert!(rank(&texts, &owners, "的 the", 10).is_empty());
        // One paper takes three places at most.
        let many = ["roots grow in soil."; 5];
        assert_eq!(rank(&many, &["a"; 5], "roots soil", 10).len(), 3);
        assert_eq!(words("Regulates regulated regulating regulation cells analyses"), ["regulat", "regulat", "regulat", "regulat", "cell", "analys"]);
    }
}
