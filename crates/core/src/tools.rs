//! The tools an agent works a library with. One table, three ways in: the
//! `wisp` command line (a tool name and flags read off its schema), the MCP
//! server (`mcp`) and the assistant inside the app. All of them end in
//! `call`, so a rule written here holds everywhere.
//!
//! Every tool here only reads. Extracting the page text of a stored PDF the
//! first time it is searched fills a cache; it changes nothing a person sees.

use crate::filter::{Filters, filter_papers};
use crate::fulltext::{file_pages, terms};
use crate::libraries::{self, LibraryInfo};
use crate::model::{FULLTEXTS, Paper, READINGS, SCREENINGS, one_of};
use crate::store::Store;
use anyhow::{Result, anyhow, bail};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;

pub struct ToolDef {
    /// What the window calls it while it runs: 「检索全文」.
    pub title: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// JSON Schema of the arguments object.
    pub input_schema: Value,
}

/// A tool that works inside one library: it also takes `library`.
fn scoped(name: &'static str, title: &'static str, description: &'static str, mut properties: Value, required: &[&str]) -> ToolDef {
    properties["library"] = json!({"type": "string", "description": "文献库的 ID 或名称；只有一个文献库时可以省略。"});
    ToolDef { name, title, description, input_schema: json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false}) }
}

/// The stored values of a status table, for a schema's `enum`.
fn keys(table: &[(&'static str, &'static str)]) -> Vec<&'static str> {
    table.iter().map(|(key, _)| *key).collect()
}

pub fn catalog() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "list_libraries",
            title: "列出文献库",
            description: "列出所有文献库：ID、名称、主题和文献数量。其他工具用 library 参数指定其中一个。",
            input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
        },
        scoped("library_status", "文献库概况", "一个文献库的概况：主题、文献数量、收录判断和阅读状态的分布、自定义词条，以及各状态值在界面上的显示名称。", json!({}), &[]),
        scoped(
            "list_papers",
            "查询文献",
            "查询文献库中的文献，返回简要记录和匹配总数。规则与界面上的列表相同：不指定 screening 时不包含判为「不相关」的文献，回收站中的文献不返回。",
            json!({
                "query": {"type": "string", "description": "在题目、作者、DOI、期刊、摘要、标签、词条和笔记中搜索；空格分开的每个词都必须出现。"},
                "screening": {"type": "string", "enum": keys(&SCREENINGS), "description": "收录判断。"},
                "reading": {"type": "string", "enum": keys(&READINGS), "description": "阅读状态。"},
                "fulltext": {"type": "string", "enum": keys(&FULLTEXTS), "description": "全文状态：stored 表示已保存 PDF。"},
                "tag": {"type": "string", "description": "带有这个标签。"},
                "year": {"type": "integer", "description": "发表年份。"},
                "sort": {"type": "string", "description": "newest（默认），或 title / year / journal / authors 加上 -asc、-desc，例如 year-desc。"},
                "limit": {"type": "integer", "description": "最多返回多少篇，默认 50，最大 500。"},
                "offset": {"type": "integer", "description": "跳过前多少篇，用于翻页。"},
            }),
            &[],
        ),
        scoped("get_paper", "读取文献", "按 ID 读取一篇文献的完整记录：书目信息、摘要、笔记、标签、自定义词条和附件。", json!({"id": {"type": "string", "description": "文献 ID，例如 list_papers 返回的 id。"}}), &["id"]),
        scoped(
            "search_fulltext",
            "检索全文",
            "在已保存的全文中检索短语或关键词，返回命中的文献、页码和逐字片段。不区分大小写和空白；整句命中排在各词分散命中之前。没有保存全文的文献不在检索范围内。",
            json!({
                "query": {"type": "string", "description": "要找的短语或关键词。"},
                "paperId": {"type": "string", "description": "只在这一篇文献中检索。"},
                "limit": {"type": "integer", "description": "最多返回多少处命中，默认 20，最大 100。"},
            }),
            &["query"],
        ),
        scoped(
            "read_fulltext",
            "阅读全文",
            "按页读取一篇文献的全文，用于核对上下文和逐字引用。一次最多 8 页。",
            json!({
                "paperId": {"type": "string", "description": "文献 ID。"},
                "page": {"type": "integer", "description": "从第几页开始，默认 1。"},
                "pages": {"type": "integer", "description": "读几页，默认 4，最大 8。"},
            }),
            &["paperId"],
        ),
        scoped(
            "find_citations",
            "查找可引用的文献",
            "为一段文字逐句找出文献库中可能支持它的文献：按词语在题目、摘要、笔记和已保存的全文里匹配，返回每句的候选文献，全文命中的附页码和原文片段。只是按词语匹配的候选，是否真的支持这句话需要读原文判断。",
            json!({
                "text": {"type": "string", "description": "要配引用的文字，可以是一句或一段。"},
                "perSentence": {"type": "integer", "description": "每句最多几篇候选，默认 3，最大 10。"},
            }),
            &["text"],
        ),
    ]
}

/// The tools of the assistant inside an open library: no choosing a library.
pub fn library_catalog() -> Vec<ToolDef> {
    let mut tools = catalog();
    tools.retain(|t| t.name != "list_libraries");
    for tool in &mut tools {
        if let Some(properties) = tool.input_schema["properties"].as_object_mut() { properties.remove("library"); }
    }
    tools
}

/// The tool `name` and whether `args` fit its schema's names.
fn checked(name: &str, args: &Value) -> Result<ToolDef, String> {
    let Some(tool) = catalog().into_iter().find(|t| t.name == name) else { return Err(format!("没有这个工具：{name}")) };
    let Some(given) = args.as_object() else { return Err("参数应为一个 JSON 对象。".into()) };
    if let Some(unknown) = given.keys().find(|key| tool.input_schema["properties"].get(key.as_str()).is_none()) {
        return Err(format!("{name} 没有参数 {unknown}。"));
    }
    if let Some(missing) = tool.input_schema["required"].as_array().into_iter().flatten().filter_map(Value::as_str).find(|key| args[*key].is_null()) {
        return Err(format!("{name} 需要参数 {missing}。"));
    }
    Ok(tool)
}

/// What a tool result may add to a model's context; a longer one is cut and says so.
pub const MAX_OUTPUT_BYTES: usize = 60_000;

/// A tool result as the text a model reads.
pub fn output_text(value: &Value) -> String {
    let text = value.to_string();
    if text.len() <= MAX_OUTPUT_BYTES { return text }
    let cut = text.floor_char_boundary(MAX_OUTPUT_BYTES);
    format!("{}\n…[结果过长，只显示了 {} 字节中的前 {cut} 字节；请缩小范围后重试]", &text[..cut], text.len())
}

/// One line for the window about a call: 「检索全文：chromium」.
pub fn describe_call(name: &str, args: &Value) -> String {
    let title = catalog().into_iter().find(|t| t.name == name).map_or(name.to_string(), |t| t.title.to_string());
    // Words first, then numbers, each group by name: the order a JSON object
    // keeps its keys in depends on how serde_json was built.
    let (mut words, mut numbers) = (vec![], vec![]);
    for (key, value) in args.as_object().into_iter().flatten().filter(|(key, _)| key.as_str() != "library") {
        match (key.as_str(), value) {
            ("page", Value::Number(n)) => numbers.push((key, format!("第 {n} 页起"))),
            (_, Value::String(text)) if !text.trim().is_empty() => words.push((key, text.split_whitespace().collect::<Vec<_>>().join(" "))),
            (_, Value::Number(n)) => numbers.push((key, format!("{key} {n}"))),
            _ => {}
        }
    }
    words.sort();
    numbers.sort();
    let detail: String = words.into_iter().chain(numbers).map(|(_, part)| part).collect::<Vec<_>>().join(" · ");
    if detail.is_empty() { return title }
    if detail.chars().count() > 80 { format!("{title}：{}…", detail.chars().take(80).collect::<String>()) } else { format!("{title}：{detail}") }
}

/// One line for the window about what came back.
pub fn describe_result(result: &Result<Value, String>) -> String {
    match result {
        Err(error) => error.clone(),
        Ok(value) => {
            if let Some(total) = value["total"].as_u64() {
                format!("{total} 条结果")
            } else if let Some(pages) = value["pages"].as_array() {
                format!("读了 {} 页", pages.len())
            } else if let Some(sentences) = value["sentences"].as_array() {
                format!("{} 句", sentences.len())
            } else {
                "完成".into()
            }
        }
    }
}

/// Run a tool against the data folder `root`. The error is a sentence for
/// whoever called: a person at a terminal or a model.
pub fn call(root: &Path, name: &str, args: &Value) -> Result<Value, String> {
    checked(name, args)?;
    if name == "list_libraries" {
        let all: Vec<Value> = libraries::list(root).iter().map(|l| json!({"id": l.id, "name": l.name, "topic": l.topic, "papers": l.paper_count})).collect();
        return Ok(json!({"libraries": all}));
    }
    let info = find_library(root, args["library"].as_str().unwrap_or(""))?;
    let mut store = Store::open(&info.dir).map_err(|e| format!("{e:#}"))?;
    call_in(&mut store, &info, name, args)
}

/// Run a library tool on a library that is already open.
pub fn call_in(store: &mut Store, info: &LibraryInfo, name: &str, args: &Value) -> Result<Value, String> {
    checked(name, args)?;
    match name {
        "find_citations" => find_citations(store, args),
        "library_status" => library_status(store, info),
        "list_papers" => list_papers(store, info, args),
        "get_paper" => get_paper(store, args),
        "search_fulltext" => search_fulltext(store, args),
        "read_fulltext" => read_fulltext(store, args),
        _ => Err(anyhow!("没有这个工具：{name}")),
    }
    .map_err(|e| format!("{e:#}"))
}

/// The library `wanted` names by id or by name; the only one when it is empty.
pub fn find_library(root: &Path, wanted: &str) -> Result<LibraryInfo, String> {
    let mut all = libraries::list(root);
    let names = |all: &[LibraryInfo]| all.iter().map(|l| format!("「{}」", l.name)).collect::<Vec<_>>().join("、");
    let wanted = wanted.trim();
    if wanted.is_empty() {
        return match all.len() {
            0 => Err(format!("{} 里还没有文献库。", root.display())),
            1 => Ok(all.remove(0)),
            _ => Err(format!("有多个文献库，请用 library 指定其中一个：{}", names(&all))),
        };
    }
    let found: Vec<usize> = (0..all.len()).filter(|&i| all[i].id == wanted || all[i].name.to_lowercase() == wanted.to_lowercase()).collect();
    match found[..] {
        [i] => Ok(all.remove(i)),
        [] if all.is_empty() => Err(format!("{} 里还没有文献库。", root.display())),
        [] => Err(format!("没有名为「{wanted}」的文献库。现有：{}", names(&all))),
        _ => Err(format!("有多个文献库叫「{wanted}」，请改用 ID。")),
    }
}

// ---- arguments

fn text<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    match &args[key] {
        Value::Null => Ok(""),
        Value::String(value) => Ok(value.trim()),
        _ => bail!("参数 {key} 应为字符串。"),
    }
}

fn number(args: &Value, key: &str, default: usize, max: usize) -> Result<usize> {
    match &args[key] {
        Value::Null => Ok(default),
        value => value.as_u64().map(|n| (n as usize).min(max)).ok_or_else(|| anyhow!("参数 {key} 应为非负整数。")),
    }
}

fn choice<'a>(args: &'a Value, key: &str, table: &[(&str, &str)]) -> Result<&'a str> {
    let value = text(args, key)?;
    if !value.is_empty() && !one_of(table, value) { bail!("参数 {key} 应为 {} 之一。", table.iter().map(|(k, _)| *k).collect::<Vec<_>>().join("、")) }
    Ok(value)
}

// ---- the tools

fn tally(papers: &[Paper], table: &[(&str, &str)], value: impl Fn(&Paper) -> &str) -> Value {
    Value::Object(table.iter().map(|(key, _)| (key.to_string(), json!(papers.iter().filter(|p| value(p) == *key).count()))).collect())
}

fn library_status(store: &mut Store, info: &LibraryInfo) -> Result<Value> {
    let papers = store.list_papers()?;
    let labels = |table: &[(&str, &str)]| Value::Object(table.iter().map(|(key, label)| (key.to_string(), json!(label))).collect());
    let fields: Vec<Value> = store.list_fields()?.iter().map(|f| json!({"name": f.name, "kind": f.kind.as_str(), "options": f.options})).collect();
    Ok(json!({
        "id": info.id,
        "name": info.name,
        "topic": info.topic,
        "folder": info.dir.display().to_string(),
        "papers": papers.len(),
        "inTrash": store.list_trash()?.len(),
        "screening": tally(&papers, &SCREENINGS, |p| &p.screening),
        "reading": tally(&papers, &READINGS, |p| &p.reading),
        "fulltext": tally(&papers, &FULLTEXTS, |p| &p.fulltext),
        "fields": fields,
        // The stored keys are not what the window shows: `excluded` reads 「相关」.
        "labels": {"screening": labels(&SCREENINGS), "reading": labels(&READINGS), "fulltext": labels(&FULLTEXTS)},
    }))
}

fn list_papers(store: &mut Store, info: &LibraryInfo, args: &Value) -> Result<Value> {
    let filters = Filters {
        search: text(args, "query")?.into(),
        screening: choice(args, "screening", &SCREENINGS)?.into(),
        reading: choice(args, "reading", &READINGS)?.into(),
        fulltext: choice(args, "fulltext", &FULLTEXTS)?.into(),
        tag: text(args, "tag")?.into(),
        year: if args["year"].is_null() { String::new() } else { number(args, "year", 0, 9999)?.to_string() },
        sort: text(args, "sort")?.into(),
        ..Default::default()
    };
    let (limit, offset) = (number(args, "limit", 50, 500)?, number(args, "offset", 0, usize::MAX)?);
    let papers = store.list_papers()?;
    let shown = filter_papers(&papers, &filters, crate::time::now_millis());
    let page: Vec<Value> = shown
        .iter()
        .skip(offset)
        .take(limit)
        .map(|&i| {
            let p = &papers[i];
            json!({"id": p.id, "title": p.title, "authors": p.short_authors(), "year": p.year, "journal": p.journal, "doi": p.doi, "pmid": p.pmid,
                "screening": p.screening, "reading": p.reading, "fulltext": p.fulltext, "starred": p.starred, "tags": p.tags})
        })
        .collect();
    Ok(json!({"library": info.name, "total": shown.len(), "offset": offset, "papers": page}))
}

fn get_paper(store: &mut Store, args: &Value) -> Result<Value> {
    let paper = store.get_paper(text(args, "id")?)?;
    let mut out = serde_json::to_value(&paper)?;
    // Custom fields by the name a person gave them, in the library's order.
    let mut fields = Map::new();
    for field in store.list_fields()? {
        if let Some(values) = paper.fields.get(&field.id) { fields.insert(field.name, json!(values)); }
    }
    out["fields"] = Value::Object(fields);
    out["link"] = json!(paper.link());
    let mut files = vec![];
    for file in store.paper_files(&paper.id)? {
        let pages = match store.fulltext_pages(&file.id)? {
            Some(Ok(pages)) => json!(pages.len()),
            _ => Value::Null,
        };
        files.push(json!({"id": file.id, "kind": file.kind, "title": file.title, "format": file.format, "sizeBytes": file.size_bytes, "path": store.file_path(&file).display().to_string(), "textPages": pages}));
    }
    out["files"] = json!(files);
    Ok(out)
}

/// Extract the page text of the full-text files that have none cached yet:
/// those of one paper, or of the whole library.
fn extract_missing(store: &mut Store, paper_id: Option<&str>) -> Result<()> {
    let files = match paper_id {
        Some(id) => store.paper_files(id)?,
        None => store.list_files()?,
    };
    for file in files.iter().filter(|f| f.kind == "fulltext") {
        if store.fulltext_pages(&file.id)?.is_some() { continue }
        let pages = file_pages(&store.file_path(file));
        store.save_fulltext_pages(&file.id, &pages)?;
    }
    Ok(())
}

/// A text ready for matching: lowercase, every whitespace character a space.
/// Byte for byte as long as the original, so a position found in it is a
/// position in the original.
fn searchable(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let mut lower = c.to_lowercase();
        match (lower.next(), lower.next()) {
            _ if c.is_whitespace() && c.len_utf8() == 1 => out.push(' '),
            (Some(l), None) if l.len_utf8() == c.len_utf8() => out.push(l),
            _ => out.push(c),
        }
    }
    out
}

/// Whether a page answers the query, how well, and where: the whole phrase
/// beats any number of scattered words, and of the words at least half must appear.
fn page_match(hay: &str, phrase: &str, words: &[String]) -> Option<(f32, usize, bool)> {
    if let Some(at) = hay.find(phrase) { return Some((1000. + hay.matches(phrase).count() as f32, at, true)) }
    let found: Vec<usize> = words.iter().filter_map(|word| hay.find(word.as_str())).collect();
    if found.is_empty() || found.len() * 2 < words.len() { return None }
    Some((found.len() as f32, found[0], false))
}

/// The passage around byte `at` of a page, as printed apart from its line breaks.
fn snippet(page: &str, at: usize) -> String {
    let start = page.floor_char_boundary(at.saturating_sub(100));
    let end = page.ceil_char_boundary((at + 220).min(page.len()));
    page[start..end].split_whitespace().collect::<Vec<_>>().join(" ")
}

fn search_fulltext(store: &mut Store, args: &Value) -> Result<Value> {
    let query = text(args, "query")?;
    let phrase = searchable(query).split_whitespace().collect::<Vec<_>>().join(" ");
    if phrase.is_empty() { bail!("请提供要检索的短语或关键词。") }
    let words = terms(query);
    let only = text(args, "paperId")?;
    let limit = number(args, "limit", 20, 100)?.max(1);
    extract_missing(store, (!only.is_empty()).then_some(only))?;
    let papers = store.list_papers()?;
    let titles: BTreeMap<&str, &str> = papers.iter().map(|p| (p.id.as_str(), p.title.as_str())).collect();
    let texts = store.all_fulltext()?;
    let mut hits = vec![];
    for (paper_id, (_, pages)) in texts.iter().filter(|(id, _)| only.is_empty() || id.as_str() == only) {
        for (n, page) in pages.iter().enumerate() {
            if let Some((score, at, exact)) = page_match(&searchable(page), &phrase, &words) { hits.push((score, paper_id.as_str(), n, snippet(page, at), exact)) }
        }
    }
    hits.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(b.1)).then(a.2.cmp(&b.2)));
    let total = hits.len();
    let shown: Vec<Value> = hits.into_iter().take(limit).map(|(_, id, n, text, exact)| json!({"paperId": id, "title": titles.get(id).copied().unwrap_or(""), "page": n + 1, "exact": exact, "snippet": text})).collect();
    Ok(json!({
        "query": query,
        "total": total,
        "hits": shown,
        // What the search could see: an answer must not pass for "the library says nothing".
        "papersSearched": texts.keys().filter(|id| only.is_empty() || id.as_str() == only).count(),
        "papersWithoutText": papers.iter().filter(|p| (only.is_empty() || p.id == only) && !texts.contains_key(&p.id)).count(),
    }))
}

fn read_fulltext(store: &mut Store, args: &Value) -> Result<Value> {
    let paper = store.get_paper(text(args, "paperId")?)?;
    extract_missing(store, Some(&paper.id))?;
    let mut failure = String::new();
    let mut found = None;
    for file in store.paper_files(&paper.id)?.into_iter().filter(|f| f.kind == "fulltext") {
        match store.fulltext_pages(&file.id)? {
            Some(Ok(pages)) => {
                found = Some(pages);
                break;
            }
            Some(Err(error)) => failure = error,
            None => {}
        }
    }
    let Some(pages) = found else {
        if failure.is_empty() { bail!("「{}」没有保存全文，只有书目信息和摘要。", paper.title) }
        bail!("「{}」的全文无法读取：{failure}", paper.title)
    };
    let from = number(args, "page", 1, usize::MAX)?.max(1);
    let count = number(args, "pages", 4, 8)?.max(1);
    if from > pages.len() { bail!("这篇文献的全文只有 {} 页。", pages.len()) }
    let read: Vec<Value> = pages.iter().enumerate().skip(from - 1).take(count).map(|(n, page)| json!({"page": n + 1, "text": page})).collect();
    Ok(json!({"paperId": paper.id, "title": paper.title, "pageCount": pages.len(), "pages": read}))
}

/// The sentences of a text: full stops of either script, question and
/// exclamation marks, semicolons and line breaks end one. A Latin full stop
/// only does when a space follows, so "0.05" and "e.g.x" stay whole.
fn sentences(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let ends = match c {
            '。' | '！' | '？' | '!' | '?' | '；' | ';' | '\n' => true,
            '.' => chars.peek().is_none_or(|next| next.is_whitespace()),
            _ => false,
        };
        if c != '\n' { current.push(c) }
        if ends {
            if !current.trim().is_empty() { out.push(current.trim().to_string()) }
            current.clear();
        }
    }
    if !current.trim().is_empty() { out.push(current.trim().to_string()) }
    out
}

const MAX_SENTENCES: usize = 40;

fn find_citations(store: &mut Store, args: &Value) -> Result<Value> {
    let text = text(args, "text")?;
    if text.is_empty() { bail!("请提供要配引用的文字。") }
    let per_sentence = number(args, "perSentence", 3, 10)?.max(1);
    extract_missing(store, None)?;
    let papers: Vec<Paper> = store.list_papers()?.into_iter().filter(|p| p.screening != "irrelevant").collect();
    let texts = store.all_fulltext()?;
    // Lowercased once: every sentence is matched against every record and page.
    let records: Vec<String> = papers.iter().map(|p| searchable(&format!("{} {} {} {}", p.title, p.abstract_text, p.tags.join(" "), p.notes))).collect();
    let pages: Vec<Vec<String>> = papers.iter().map(|p| texts.get(&p.id).map(|(_, pages)| pages.iter().map(|page| searchable(page)).collect()).unwrap_or_default()).collect();
    let all = sentences(text);
    let mut out = vec![];
    for sentence in all.iter().take(MAX_SENTENCES) {
        let words = terms(sentence);
        let mut candidates = vec![];
        // A sentence of one or two words says too little to match on.
        if words.len() >= 3 {
            for (i, paper) in papers.iter().enumerate() {
                let in_record = words.iter().filter(|word| records[i].contains(word.as_str())).count();
                // The page holding the most of the sentence's words, and where its first one stands.
                let best = pages[i].iter().enumerate().map(|(n, page)| (words.iter().filter(|word| page.contains(word.as_str())).count(), n)).max_by_key(|&(count, n)| (count, std::cmp::Reverse(n)));
                let (count, page) = match best {
                    Some((count, n)) if count > in_record => (count, Some(n)),
                    _ => (in_record, None),
                };
                if count * 2 < words.len() { continue }
                let mut candidate = json!({"paperId": paper.id, "title": paper.title, "year": paper.year, "matched": count, "of": words.len(), "in": if page.is_some() { "fulltext" } else { "record" }});
                if let Some(n) = page {
                    let original = &texts[&paper.id].1[n];
                    let at = words.iter().filter_map(|word| pages[i][n].find(word.as_str())).min().unwrap_or(0);
                    candidate["page"] = json!(n + 1);
                    candidate["snippet"] = json!(snippet(original, at));
                }
                candidates.push((count, candidate));
            }
        }
        candidates.sort_by_key(|(count, _)| std::cmp::Reverse(*count));
        out.push(json!({"sentence": sentence, "candidates": candidates.into_iter().take(per_sentence).map(|(_, c)| c).collect::<Vec<_>>()}));
    }
    Ok(json!({
        "sentences": out,
        "sentencesSkipped": all.len().saturating_sub(MAX_SENTENCES),
        "papersSearched": papers.len(),
        "papersWithoutText": papers.iter().filter(|p| !texts.contains_key(&p.id)).count(),
    }))
}

// ---- the command line

/// `paperId` as a flag: `paper-id`.
fn flag_name(key: &str) -> String {
    let mut out = String::new();
    for c in key.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A tool's arguments from command-line words: `--paper-id P-1 --limit 5`,
/// `--limit=5`, `-l 名称` for `--library`; a bare word fills the next
/// required argument, so `get_paper P-1` works.
pub fn cli_args(schema: &Value, words: &[String]) -> Result<Value, String> {
    let empty = Map::new();
    let properties = schema["properties"].as_object().unwrap_or(&empty);
    let mut required = schema["required"].as_array().into_iter().flatten().filter_map(Value::as_str);
    let mut out = Map::new();
    let mut words = words.iter();
    while let Some(word) = words.next() {
        let (key, mut value) = if word == "-l" {
            ("library".to_string(), None)
        } else if let Some(flag) = word.strip_prefix("--") {
            let (flag, value) = flag.split_once('=').map_or((flag, None), |(f, v)| (f, Some(v.to_string())));
            let Some(key) = properties.keys().find(|key| flag_name(key) == flag || key.as_str() == flag) else {
                return Err(format!("没有参数 --{flag}。可用：{}", properties.keys().map(|k| format!("--{}", flag_name(k))).collect::<Vec<_>>().join(" ")));
            };
            (key.clone(), value)
        } else {
            let Some(key) = required.find(|key| !out.contains_key(*key)) else { return Err(format!("多余的参数：{word}")) };
            (key.to_string(), Some(word.clone()))
        };
        let kind = properties.get(&key).and_then(|p| p["type"].as_str()).unwrap_or("string");
        if kind == "boolean" && value.is_none() { value = Some("true".into()) }
        let Some(value) = value.or_else(|| words.next().cloned()) else { return Err(format!("--{} 后面需要一个值。", flag_name(&key))) };
        let parsed = match kind {
            "integer" => json!(value.parse::<i64>().map_err(|_| format!("--{} 应为整数：{value}", flag_name(&key)))?),
            "boolean" => json!(value == "true"),
            _ => json!(value),
        };
        out.insert(key, parsed);
    }
    Ok(Value::Object(out))
}

/// What `wisp <tool> --help` prints.
pub fn usage(tool: &ToolDef) -> String {
    let required: Vec<&str> = tool.input_schema["required"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let mut out = format!("wisp {}{}\n\n{}\n", tool.name, required.iter().map(|r| format!(" <{}>", flag_name(r))).collect::<String>(), tool.description);
    for (key, property) in tool.input_schema["properties"].as_object().into_iter().flatten() {
        let mut line = format!("\n  --{} {}", flag_name(key), property["description"].as_str().unwrap_or(""));
        if let Some(values) = property["enum"].as_array() { line.push_str(&format!("（{}）", values.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" | "))) }
        out.push_str(&line);
    }
    out.push('\n');
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::{FieldDef, new_id};
    use std::path::PathBuf;

    /// A data folder with one library: two papers, the first with a two-page text.
    pub(crate) fn sample() -> (PathBuf, String) {
        let root = std::env::temp_dir().join(format!("wisp-tools-{}", new_id("T")));
        let info = libraries::create(&root, "根", "根尖发育", "green").unwrap();
        let mut store = Store::open(&info.dir).unwrap();
        let mut species = FieldDef { name: "物种".into(), ..Default::default() };
        store.save_field(&mut species).unwrap();
        let mut atlas = Paper { title: "Root stem cell niche".into(), doi: "10.1000/root".into(), authors: "Zhang Wei; Li Na; Wang Fang".into(), year: Some(2021), abstract_text: "WOX5 maintains the quiescent centre.".into(), tags: vec!["atlas".into()], ..Default::default() };
        atlas.fields.insert(species.id.clone(), vec!["Arabidopsis".into()]);
        let atlas = store.add_paper(atlas).unwrap();
        let mut leaf = store.add_paper(Paper { title: "Leaf development".into(), year: Some(2019), ..Default::default() }).unwrap();
        leaf.decide("irrelevant", "");
        store.save_paper(&mut leaf).unwrap();
        // A text file is one page; the two pages a PDF would give go straight into the cache.
        let source = root.join("root.txt");
        std::fs::write(&source, "placeholder").unwrap();
        let file = store.import_file(&atlas.id, &source, "fulltext").unwrap();
        store.save_fulltext_pages(&file.id, &Ok(vec!["Introduction.\nThe root tip of Arabidopsis.".into(), "Libraries were prepared with\n10x Genomics Chromium. WOX5 marks the quiescent centre.".into()])).unwrap();
        (root, atlas.id)
    }

    #[test]
    fn libraries_and_status() {
        let (root, _) = sample();
        let listed = call(&root, "list_libraries", &json!({})).unwrap();
        assert_eq!((listed["libraries"][0]["name"].as_str(), listed["libraries"][0]["papers"].as_u64()), (Some("根"), Some(2)));
        // The only library needs no name; by name or id it is the same one.
        let status = call(&root, "library_status", &json!({})).unwrap();
        assert_eq!(call(&root, "library_status", &json!({"library": "根"})).unwrap(), status);
        assert_eq!((status["papers"].as_u64(), status["screening"]["irrelevant"].as_u64(), status["fulltext"]["stored"].as_u64()), (Some(2), Some(1), Some(1)));
        assert_eq!((status["labels"]["screening"]["excluded"].as_str(), status["fields"][0]["name"].as_str()), (Some("相关"), Some("物种")));

        assert!(call(&root, "library_status", &json!({"library": "叶"})).unwrap_err().contains("「根」"));
        libraries::create(&root, "叶", "", "blue").unwrap();
        assert!(call(&root, "library_status", &json!({})).unwrap_err().contains("多个文献库"));
        assert_eq!(call(&root, "library_status", &json!({"library": "叶"})).unwrap()["papers"], 0);
        assert!(find_library(&root.join("nowhere"), "").unwrap_err().contains("还没有文献库"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn papers() {
        let (root, id) = sample();
        // Like the window's list: the paper marked irrelevant shows only when asked for.
        let all = call(&root, "list_papers", &json!({})).unwrap();
        assert_eq!((all["total"].as_u64(), all["papers"][0]["authors"].as_str()), (Some(1), Some("Zhang Wei 等")));
        assert_eq!(call(&root, "list_papers", &json!({"screening": "irrelevant"})).unwrap()["papers"][0]["title"], "Leaf development");
        assert_eq!(call(&root, "list_papers", &json!({"query": "wox5 niche", "year": 2021})).unwrap()["total"], 1);
        assert_eq!(call(&root, "list_papers", &json!({"query": "wox5", "year": 2019})).unwrap()["total"], 0);
        let paged = call(&root, "list_papers", &json!({"limit": 1, "offset": 1})).unwrap();
        assert_eq!((paged["total"].as_u64(), paged["papers"].as_array().map(Vec::len)), (Some(1), Some(0)));

        let paper = call(&root, "get_paper", &json!({"id": id})).unwrap();
        assert_eq!((paper["doi"].as_str(), paper["fields"]["物种"][0].as_str(), paper["link"].as_str()), (Some("10.1000/root"), Some("Arabidopsis"), Some("https://doi.org/10.1000/root")));
        assert_eq!((paper["files"][0]["title"].as_str(), paper["files"][0]["textPages"].as_u64()), (Some("root.txt"), Some(2)));
        assert!(Path::new(paper["files"][0]["path"].as_str().unwrap()).exists());

        for (name, args, message) in [
            ("get_paper", json!({"id": "P-none"}), "文献不存在"),
            ("get_paper", json!({}), "需要参数 id"),
            ("list_papers", json!({"screening": "maybe"}), "pending"),
            ("list_papers", json!({"limit": "ten"}), "非负整数"),
            ("list_papers", json!({"colour": "red"}), "没有参数 colour"),
            ("list_papers", json!([]), "JSON 对象"),
            ("drop_everything", json!({}), "没有这个工具"),
        ] {
            let error = call(&root, name, &args).unwrap_err();
            assert!(error.contains(message), "{name}: {error}");
        }
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fulltext() {
        let (root, id) = sample();
        // The phrase runs over a line break and differs in case.
        let found = call(&root, "search_fulltext", &json!({"query": "prepared with 10X genomics"})).unwrap();
        let hit = &found["hits"][0];
        assert_eq!((found["total"].as_u64(), hit["page"].as_u64(), hit["exact"].as_bool(), hit["paperId"].as_str()), (Some(1), Some(2), Some(true), Some(id.as_str())));
        assert!(hit["snippet"].as_str().unwrap().contains("prepared with 10x Genomics Chromium."));
        assert_eq!((found["papersSearched"].as_u64(), found["papersWithoutText"].as_u64()), (Some(1), Some(1)));
        // Scattered words: each page holds one of the two. One word out of four is not a hit.
        let loose = call(&root, "search_fulltext", &json!({"query": "WOX5 Arabidopsis"})).unwrap();
        assert_eq!((loose["total"].as_u64(), loose["hits"][0]["exact"].as_bool()), (Some(2), Some(false)));
        assert_eq!(call(&root, "search_fulltext", &json!({"query": "wox5 auxin gradient cytokinin"})).unwrap()["total"], 0);
        assert_eq!(call(&root, "search_fulltext", &json!({"query": "chromium", "paperId": "P-other"})).unwrap()["total"], 0);
        assert!(call(&root, "search_fulltext", &json!({"query": "  "})).is_err());

        let read = call(&root, "read_fulltext", &json!({"paperId": id, "page": 2})).unwrap();
        assert_eq!((read["pageCount"].as_u64(), read["pages"].as_array().map(Vec::len), read["pages"][0]["page"].as_u64()), (Some(2), Some(1), Some(2)));
        assert!(read["pages"][0]["text"].as_str().unwrap().starts_with("Libraries were prepared"));
        assert!(call(&root, "read_fulltext", &json!({"paperId": id, "page": 3})).unwrap_err().contains("只有 2 页"));
        let leaf = call(&root, "list_papers", &json!({"screening": "irrelevant"})).unwrap()["papers"][0]["id"].as_str().unwrap().to_string();
        assert!(call(&root, "read_fulltext", &json!({"paperId": leaf})).unwrap_err().contains("没有保存全文"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn citations_for_a_text() {
        let (root, id) = sample();
        assert_eq!(sentences("Roots grow (p < 0.05). 根尖有干细胞。WOX5 is key;\nDone"), ["Roots grow (p < 0.05).", "根尖有干细胞。", "WOX5 is key;", "Done"]);
        let found = call(&root, "find_citations", &json!({"text": "Libraries were prepared with the Chromium platform. WOX5 maintains the quiescent centre of the root. Too short."})).unwrap();
        let sentence = |n: usize| &found["sentences"][n];
        assert_eq!(found["sentences"].as_array().unwrap().len(), 3);
        // The methods sentence is only in the full text, on page 2; the second is the abstract.
        assert_eq!((sentence(0)["candidates"][0]["paperId"].as_str(), sentence(0)["candidates"][0]["in"].as_str(), sentence(0)["candidates"][0]["page"].as_u64()), (Some(id.as_str()), Some("fulltext"), Some(2)));
        assert!(sentence(0)["candidates"][0]["snippet"].as_str().unwrap().contains("Libraries were prepared"));
        assert_eq!((sentence(1)["candidates"][0]["in"].as_str(), sentence(1)["candidates"][0]["page"].as_u64()), (Some("record"), None));
        assert_eq!(sentence(2)["candidates"], json!([]));
        // The paper marked irrelevant is never a candidate.
        assert_eq!((found["papersSearched"].as_u64(), found["papersWithoutText"].as_u64()), (Some(1), Some(0)));
        assert_eq!(call(&root, "find_citations", &json!({"text": "Unrelated sentence about volcanic eruptions in Iceland."})).unwrap()["sentences"][0]["candidates"], json!([]));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn what_the_window_shows() {
        assert_eq!(describe_call("search_fulltext", &json!({"query": "10x  Genomics", "limit": 5})), "检索全文：10x Genomics · limit 5");
        assert_eq!(describe_call("read_fulltext", &json!({"paperId": "P-1", "page": 3})), "阅读全文：P-1 · 第 3 页起");
        assert_eq!(describe_call("library_status", &json!({})), "文献库概况");
        assert_eq!(describe_result(&Ok(json!({"total": 3}))), "3 条结果");
        assert_eq!(describe_result(&Ok(json!({"pages": [1, 2]}))), "读了 2 页");
        assert_eq!(describe_result(&Err("没有".into())), "没有");
        let long = json!({"text": "页".repeat(MAX_OUTPUT_BYTES)});
        let shown = output_text(&long);
        assert!(shown.len() < MAX_OUTPUT_BYTES + 200 && shown.contains("结果过长"));
        assert_eq!(output_text(&json!({"a": 1})), r#"{"a":1}"#);
        // Inside a library there is no library to choose.
        let inside = library_catalog();
        assert!(inside.iter().all(|t| t.name != "list_libraries" && t.input_schema["properties"].get("library").is_none()));
        assert_eq!(inside.len(), catalog().len() - 1);
    }

    #[test]
    fn text_is_extracted_on_first_use() {
        let root = std::env::temp_dir().join(format!("wisp-tools-{}", new_id("T")));
        let info = libraries::create(&root, "L", "", "blue").unwrap();
        let mut store = Store::open(&info.dir).unwrap();
        let paper = store.add_paper(Paper { title: "Notes".into(), ..Default::default() }).unwrap();
        let source = root.join("notes.md");
        std::fs::write(&source, "Auxin sets the gradient.").unwrap();
        store.import_file(&paper.id, &source, "fulltext").unwrap();
        drop(store);
        assert_eq!(call(&root, "search_fulltext", &json!({"query": "auxin"})).unwrap()["hits"][0]["snippet"], "Auxin sets the gradient.");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn matching_keeps_positions() {
        // Characters whose lowercase form has another length stay as they are.
        for text in ["İstanbul\tROOT\u{3000}tip", "ǅ and ß", "根尖 WOX5"] {
            assert_eq!(searchable(text).len(), text.len(), "{text}");
        }
        assert_eq!(searchable("A\nB\tc"), "a b c");
        let page = format!("{}根尖 marker", "é".repeat(120));
        let at = searchable(&page).find("marker").unwrap();
        assert!(snippet(&page, at).ends_with("根尖 marker"));
    }

    #[test]
    fn command_line_arguments() {
        let tools = catalog();
        let schema = |name: &str| &tools.iter().find(|t| t.name == name).unwrap().input_schema;
        let words = |line: &str| line.split(' ').map(String::from).collect::<Vec<_>>();
        assert_eq!(cli_args(schema("get_paper"), &words("P-1 -l 根")).unwrap(), json!({"id": "P-1", "library": "根"}));
        assert_eq!(cli_args(schema("search_fulltext"), &words("chromium --paper-id P-1 --limit=5")).unwrap(), json!({"query": "chromium", "paperId": "P-1", "limit": 5}));
        assert_eq!(cli_args(schema("list_papers"), &[]).unwrap(), json!({}));
        for (name, line, message) in [("list_papers", "stray", "多余的参数"), ("list_papers", "--limit ten", "应为整数"), ("list_papers", "--limit", "需要一个值"), ("list_papers", "--colour red", "没有参数 --colour"), ("get_paper", "--paper-id 1", "可用：--id --library")] {
            let error = cli_args(schema(name), &words(line)).unwrap_err();
            assert!(error.contains(message), "{line}: {error}");
        }
        let help = usage(tools.iter().find(|t| t.name == "read_fulltext").unwrap());
        assert!(help.starts_with("wisp read_fulltext <paper-id>\n") && help.contains("--pages 读几页"));
    }

    #[test]
    fn every_tool_is_callable() {
        let (root, _) = sample();
        for tool in catalog() {
            assert_eq!(tool.input_schema["type"], "object", "{}", tool.name);
            // A missing argument is reported by `call` itself; anything else means the name is not wired up.
            if let Err(error) = call(&root, tool.name, &json!({})) { assert!(error.contains("需要参数"), "{}: {error}", tool.name) }
        }
        std::fs::remove_dir_all(root).ok();
    }
}
