//! Searching the literature databases: which URLs to ask and how to read
//! what comes back. No HTTP here; the caller hands in the function that
//! fetches a URL, so the same code runs in the app, in `wisp` and in tests.
//!
//! PubMed, Europe PMC, Crossref and arXiv need no key. Each takes its own
//! query syntax (`syntax`), which is what a saved search stores.

use crate::lookup::{crossref_work, europepmc_hit};
use crate::model::{Paper, normalize_doi, strip_markup};
use serde_json::Value;

/// The databases: (stored key, name shown).
pub const KINDS: [(&str, &str); 4] = [("pubmed", "PubMed"), ("europepmc", "Europe PMC"), ("crossref", "Crossref"), ("arxiv", "arXiv")];

/// The most records one search may bring back.
pub const MAX_RESULTS: usize = 200;

/// GET a URL and return the body.
pub type Fetch<'a> = &'a dyn Fn(&str) -> Result<String, String>;

/// What a search found: how many records the database holds for the query
/// (when it says), and the newest of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    pub total: Option<u64>,
    pub papers: Vec<Paper>,
}

/// How a query is written for a database, for whoever composes one.
pub fn syntax(kind: &str) -> &'static str {
    match kind {
        "pubmed" => "PubMed 检索式：布尔运算符 AND / OR / NOT 要大写，用括号分组，短语加引号；可以用字段标签，如 \"single cell\"[Title/Abstract]、Arabidopsis[MeSH Terms]、2020:2025[dp]；词尾 * 表示截词。",
        "europepmc" => "Europe PMC 检索式：布尔运算符 AND / OR / NOT 要大写，用括号分组，短语加引号；字段写在前面，如 TITLE:\"root\"、ABSTRACT:\"single cell\"、PUB_YEAR:[2020 TO 2025]、SRC:PPR（只要预印本）。",
        "crossref" => "Crossref 按词语匹配题目、作者和期刊，不支持布尔运算符和字段：只写 3 到 6 个最有区分度的词。",
        "arxiv" => "arXiv 检索式：字段前缀 ti:（题目）、abs:（摘要）、au:（作者）、cat:（分类，如 cs.CL）、all:（全部）；布尔运算符 AND / OR / ANDNOT 要大写，用括号分组，短语加引号。",
        _ => "",
    }
}

fn encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { out.push(b as char) } else { out.push_str(&format!("%{b:02X}")) }
    }
    out
}

/// Run `query` against one database and return up to `limit` of the newest records.
pub fn search(kind: &str, query: &str, limit: usize, fetch: Fetch) -> Result<Found, String> {
    let query = query.trim();
    if query.is_empty() { return Err("请填写检索式。".into()) }
    let limit = limit.clamp(1, MAX_RESULTS);
    let json = |url: &str| -> Result<Value, String> { serde_json::from_str(&fetch(url)?).map_err(|e| format!("无法解析返回内容：{e}")) };
    let mut found = match kind {
        "pubmed" => {
            let base = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils";
            let hits = json(&format!("{base}/esearch.fcgi?db=pubmed&retmode=json&sort=pub+date&retmax={limit}&term={}", encode(query)))?;
            let result = &hits["esearchresult"];
            // PubMed answers a query it cannot read with an error inside a normal reply.
            if let Some(error) = result["ERROR"].as_str() { return Err(format!("PubMed 没有接受这个检索式：{error}")) }
            let ids: Vec<&str> = result["idlist"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            let total = result["count"].as_str().and_then(|n| n.parse().ok());
            let papers = if ids.is_empty() { vec![] } else { pubmed_papers(&fetch(&format!("{base}/efetch.fcgi?db=pubmed&retmode=xml&id={}", ids.join(",")))?) };
            Found { total, papers }
        }
        "europepmc" => {
            let body = json(&format!("https://www.ebi.ac.uk/europepmc/webservices/rest/search?format=json&resultType=core&sort={}&pageSize={limit}&query={}", encode("P_PDATE_D desc"), encode(query)))?;
            Found { total: body["hitCount"].as_u64(), papers: body["resultList"]["result"].as_array().into_iter().flatten().filter_map(europepmc_hit).collect() }
        }
        "crossref" => {
            let body = json(&format!("https://api.crossref.org/works?rows={limit}&select={}&query.bibliographic={}", encode("DOI,title,author,published,published-print,published-online,issued,container-title,abstract,type,URL,publisher"), encode(query)))?;
            Found { total: body["message"]["total-results"].as_u64(), papers: body["message"]["items"].as_array().into_iter().flatten().filter_map(crossref_work).collect() }
        }
        "arxiv" => arxiv_found(&fetch(&format!("https://export.arxiv.org/api/query?sortBy=submittedDate&sortOrder=descending&max_results={limit}&search_query={}", encode(query)))?),
        other => return Err(format!("不支持的来源：{other}。可用：{}", KINDS.iter().map(|(key, _)| *key).collect::<Vec<_>>().join("、"))),
    };
    let name = KINDS.iter().find(|(key, _)| *key == kind).map_or(kind, |(_, name)| name);
    for paper in &mut found.papers {
        paper.source = name.to_string();
    }
    found.papers.truncate(limit);
    Ok(found)
}

// ---- XML, as far as PubMed and arXiv need it

/// The attributes and the inside of every `<tag …>…</tag>` in `xml`, in
/// order. Enough for the flat records these services return; an element
/// nested in one of its own kind is not handled.
fn elements<'a>(xml: &'a str, tag: &str) -> Vec<(&'a str, &'a str)> {
    let (open, close) = (format!("<{tag}"), format!("</{tag}>"));
    let mut found = vec![];
    let mut rest = xml;
    while let Some(at) = rest.find(&open) {
        let after = &rest[at + open.len()..];
        // `<Author` must not match `<AuthorList`.
        if !after.starts_with(|c: char| c == '>' || c == '/' || c.is_whitespace()) {
            rest = after;
            continue;
        }
        let Some(end) = after.find('>') else { break };
        let attributes = &after[..end];
        let body = &after[end + 1..];
        if attributes.ends_with('/') {
            rest = body;
            continue;
        }
        let Some(stop) = body.find(&close) else { break };
        found.push((attributes, &body[..stop]));
        rest = &body[stop + close.len()..];
    }
    found
}

fn element<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    elements(xml, tag).first().map(|(_, inside)| *inside)
}

/// The text of the first `<tag>`, without markup.
fn text_of(xml: &str, tag: &str) -> String {
    element(xml, tag).map(strip_markup).unwrap_or_default()
}

fn attribute(attributes: &str, name: &str) -> Option<String> {
    let at = attributes.find(&format!("{name}=\""))?;
    let value = &attributes[at + name.len() + 2..];
    Some(value[..value.find('"')?].to_string())
}

fn year_in(text: &str) -> Option<i32> {
    let digits: String = text.chars().skip_while(|c| !c.is_ascii_digit()).take(4).collect();
    digits.parse().ok().filter(|y| (1800..=2200).contains(y))
}

/// The records of a PubMed `efetch` reply.
pub fn pubmed_papers(xml: &str) -> Vec<Paper> {
    let mut papers = vec![];
    for (_, article) in elements(xml, "PubmedArticle") {
        let title = text_of(article, "ArticleTitle").trim_end_matches('.').to_string();
        let pmid = text_of(article, "PMID");
        if title.is_empty() || pmid.is_empty() { continue }
        let authors: Vec<String> = element(article, "AuthorList")
            .map(|list| {
                elements(list, "Author")
                    .iter()
                    .filter_map(|(_, author)| {
                        let (family, given) = (text_of(author, "LastName"), text_of(author, "ForeName"));
                        let name = if family.is_empty() { text_of(author, "CollectiveName") } else if given.is_empty() { family } else { format!("{family}, {given}") };
                        (!name.is_empty()).then_some(name)
                    })
                    .collect()
            })
            .unwrap_or_default();
        // A structured abstract comes in labelled parts.
        let parts: Vec<String> = element(article, "Abstract")
            .map(|block| {
                elements(block, "AbstractText")
                    .iter()
                    .map(|(attributes, part)| match attribute(attributes, "Label") {
                        Some(label) if !label.is_empty() => format!("{label}: {}", strip_markup(part)),
                        _ => strip_markup(part),
                    })
                    .filter(|part| !part.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        // The article's own identifiers: the list in PubmedData, before the references and theirs.
        let own = element(article, "PubmedData").map(|data| data.split("<ReferenceList").next().unwrap_or(data)).unwrap_or("");
        let doi = elements(article, "ELocationID")
            .into_iter()
            .chain(elements(own, "ArticleId"))
            .find(|(attributes, _)| attributes.contains("\"doi\""))
            .map(|(_, value)| normalize_doi(&strip_markup(value)))
            .unwrap_or_default();
        let year = element(article, "PubDate").and_then(year_in).or_else(|| element(article, "ArticleDate").and_then(year_in));
        let preprint = elements(article, "PublicationType").iter().any(|(_, kind)| kind.trim() == "Preprint");
        papers.push(Paper {
            title,
            pmid,
            doi,
            authors: authors.join("; "),
            journal: element(article, "Journal").map(|journal| text_of(journal, "Title")).unwrap_or_default(),
            year,
            abstract_text: parts.join("\n"),
            publication_status: if preprint { "preprint" } else { "published" }.into(),
            ..Default::default()
        });
    }
    papers
}

/// An arXiv API reply (Atom).
pub fn arxiv_found(xml: &str) -> Found {
    let mut papers = vec![];
    for (_, entry) in elements(xml, "entry") {
        let title = text_of(entry, "title");
        // `http://arxiv.org/abs/1706.03762v7`: the id without its version.
        let id = text_of(entry, "id");
        let number = id.rsplit("/abs/").next().unwrap_or("").trim();
        let number = number.rfind('v').filter(|at| number[at + 1..].bytes().all(|b| b.is_ascii_digit()) && *at + 1 < number.len()).map_or(number, |at| &number[..at]);
        if title.is_empty() || number.is_empty() { continue }
        // A published version's DOI when the authors gave one, else arXiv's own.
        let doi = element(entry, "arxiv:doi").map(strip_markup).filter(|doi| !doi.is_empty());
        papers.push(Paper {
            title,
            authors: elements(entry, "author").iter().map(|(_, author)| text_of(author, "name")).filter(|name| !name.is_empty()).collect::<Vec<_>>().join("; "),
            year: year_in(&text_of(entry, "published")),
            journal: text_of(entry, "arxiv:journal_ref"),
            publication_status: if doi.is_some() { "published" } else { "preprint" }.into(),
            doi: normalize_doi(&doi.unwrap_or_else(|| format!("10.48550/arxiv.{number}"))),
            url: format!("https://arxiv.org/abs/{number}"),
            abstract_text: text_of(entry, "summary"),
            ..Default::default()
        });
    }
    Found { total: element(xml, "opensearch:totalResults").and_then(|n| n.trim().parse().ok()), papers }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const PUBMED: &str = r#"<?xml version="1.0" ?><PubmedArticleSet>
<PubmedArticle><MedlineCitation Status="MEDLINE"><PMID Version="1">31178400</PMID>
<Article PubModel="Print-Electronic"><Journal><ISSN>1534-5807</ISSN><JournalIssue><PubDate><Year>2019</Year><Month>Mar</Month></PubDate></JournalIssue><Title>Developmental cell</Title></Journal>
<ArticleTitle>A Single-Cell RNA Sequencing Profiles the Developmental Landscape of <i>Arabidopsis</i> Root.</ArticleTitle>
<ELocationID EIdType="pii" ValidYN="Y">S1534-5807(19)30145-4</ELocationID><ELocationID EIdType="doi" ValidYN="Y">10.1016/j.devcel.2019.02.022</ELocationID>
<Abstract><AbstractText Label="BACKGROUND">Roots &amp; cells.</AbstractText><AbstractText Label="RESULTS">We profiled <sup>7,000</sup> cells.</AbstractText></Abstract>
<AuthorList CompleteYN="Y"><Author ValidYN="Y"><LastName>Zhang</LastName><ForeName>Tian-Qi</ForeName><Initials>TQ</Initials></Author><Author><CollectiveName>Root Consortium</CollectiveName></Author></AuthorList>
<PublicationTypeList><PublicationType UI="D016428">Journal Article</PublicationType></PublicationTypeList></Article></MedlineCitation>
<PubmedData><ArticleIdList><ArticleId IdType="pubmed">31178400</ArticleId></ArticleIdList>
<ReferenceList><Reference><ArticleIdList><ArticleId IdType="doi">10.9999/cited</ArticleId></ArticleIdList></Reference></ReferenceList></PubmedData></PubmedArticle>
<PubmedArticle><MedlineCitation><PMID>2</PMID><Article><Journal><JournalIssue><PubDate><MedlineDate>2021 Jan-Feb</MedlineDate></PubDate></JournalIssue><Title>bioRxiv</Title></Journal>
<ArticleTitle>No DOI in the article</ArticleTitle><PublicationTypeList><PublicationType>Preprint</PublicationType></PublicationTypeList></Article></MedlineCitation>
<PubmedData><ArticleIdList><ArticleId IdType="pubmed">2</ArticleId><ArticleId IdType="doi">10.1101/2021.01.01.1</ArticleId></ArticleIdList></PubmedData></PubmedArticle>
<PubmedArticle><MedlineCitation><PMID>3</PMID><Article><ArticleTitle></ArticleTitle></Article></MedlineCitation></PubmedArticle>
</PubmedArticleSet>"#;

    #[test]
    fn pubmed_records() {
        let papers = pubmed_papers(PUBMED);
        assert_eq!(papers.len(), 2);
        let p = &papers[0];
        assert_eq!(p.title, "A Single-Cell RNA Sequencing Profiles the Developmental Landscape of Arabidopsis Root");
        assert_eq!((p.pmid.as_str(), p.doi.as_str(), p.year), ("31178400", "10.1016/j.devcel.2019.02.022", Some(2019)));
        assert_eq!((p.authors.as_str(), p.journal.as_str()), ("Zhang, Tian-Qi; Root Consortium", "Developmental cell"));
        assert_eq!(p.abstract_text, "BACKGROUND: Roots & cells.\nRESULTS: We profiled 7,000 cells.");
        assert_eq!(p.publication_status, "published");
        // The DOI of the record, never one from its reference list.
        assert_eq!((papers[1].doi.as_str(), papers[1].year, papers[1].publication_status.as_str()), ("10.1101/2021.01.01.1", Some(2021), "preprint"));
    }

    #[test]
    fn arxiv_records() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>ArXiv Query</title><opensearch:totalResults xmlns:opensearch="http://a9.com/-/spec/opensearch/1.1/">1234</opensearch:totalResults>
<entry><id>http://arxiv.org/abs/1706.03762v7</id><published>2017-06-12T17:57:34Z</published><title>Attention Is All
  You Need</title><summary>  The dominant sequence transduction models.
</summary><author><name>Ashish Vaswani</name></author><author><name>Noam Shazeer</name></author><link href="http://arxiv.org/abs/1706.03762v7" rel="alternate" type="text/html"/></entry>
<entry><id>http://arxiv.org/abs/cs/9901001v1</id><published>1999-01-02T00:00:00Z</published><title>Old style id</title><summary>S</summary><arxiv:doi xmlns:arxiv="http://arxiv.org/schemas/atom">10.1000/Published</arxiv:doi><arxiv:journal_ref xmlns:arxiv="http://arxiv.org/schemas/atom">J. Old 1</arxiv:journal_ref></entry></feed>"#;
        let found = arxiv_found(xml);
        assert_eq!((found.total, found.papers.len()), (Some(1234), 2));
        let p = &found.papers[0];
        assert_eq!((p.title.as_str(), p.authors.as_str(), p.year), ("Attention Is All You Need", "Ashish Vaswani; Noam Shazeer", Some(2017)));
        assert_eq!((p.doi.as_str(), p.url.as_str(), p.publication_status.as_str()), ("10.48550/arxiv.1706.03762", "https://arxiv.org/abs/1706.03762", "preprint"));
        assert_eq!(p.abstract_text, "The dominant sequence transduction models.");
        let old = &found.papers[1];
        assert_eq!((old.doi.as_str(), old.url.as_str(), old.journal.as_str(), old.publication_status.as_str()), ("10.1000/published", "https://arxiv.org/abs/cs/9901001", "J. Old 1", "published"));
    }

    #[test]
    fn searches_ask_the_right_urls() {
        let asked = RefCell::new(vec![]);
        let fetch = |url: &str| -> Result<String, String> {
            asked.borrow_mut().push(url.to_string());
            Ok(if url.contains("esearch") {
                r#"{"esearchresult":{"count":"321","idlist":["31178400","2"]}}"#.to_string()
            } else if url.contains("efetch") {
                PUBMED.to_string()
            } else if url.contains("europepmc") {
                r#"{"hitCount":7,"resultList":{"result":[{"title":"Root atlas.","pubYear":"2020","pmid":"9","authorString":"Li N."}]}}"#.to_string()
            } else if url.contains("crossref") {
                r#"{"message":{"total-results":50,"items":[{"title":["A"],"DOI":"10.1000/A"},{"title":["B"],"DOI":"10.1000/b"},{"DOI":"10.1000/untitled"}]}}"#.to_string()
            } else {
                "<feed></feed>".to_string()
            })
        };
        let pubmed = search("pubmed", " \"single cell\"[Title] AND root ", 20, &fetch).unwrap();
        assert_eq!((pubmed.total, pubmed.papers.len(), pubmed.papers[0].source.as_str()), (Some(321), 2, "PubMed"));
        assert!(asked.borrow()[0].ends_with("retmax=20&term=%22single%20cell%22%5BTitle%5D%20AND%20root"), "{}", asked.borrow()[0]);
        assert!(asked.borrow()[1].ends_with("&id=31178400,2"));

        let pmc = search("europepmc", "TITLE:root", 500, &fetch).unwrap();
        assert_eq!((pmc.total, pmc.papers[0].title.as_str(), pmc.papers[0].source.as_str()), (Some(7), "Root atlas", "Europe PMC"));
        assert!(asked.borrow()[2].contains("pageSize=200&query=TITLE%3Aroot"));

        // The limit holds even when a service sends more than it was asked for.
        let crossref = search("crossref", "root atlas", 1, &fetch).unwrap();
        assert_eq!((crossref.total, crossref.papers.len(), crossref.papers[0].doi.as_str()), (Some(50), 1, "10.1000/a"));
        assert_eq!(search("arxiv", "all:root", 5, &fetch).unwrap(), Found::default());
        assert!(asked.borrow()[4].starts_with("https://export.arxiv.org/api/query?") && asked.borrow()[4].ends_with("search_query=all%3Aroot"));

        assert!(search("pubmed", "  ", 5, &fetch).unwrap_err().contains("检索式"));
        assert!(search("scholar", "x", 5, &fetch).unwrap_err().contains("pubmed"));
        let refused = |_: &str| -> Result<String, String> { Ok(r#"{"esearchresult":{"ERROR":"Invalid field"}}"#.to_string()) };
        assert!(search("pubmed", "x[zz]", 5, &refused).unwrap_err().contains("Invalid field"));
        let offline = |_: &str| -> Result<String, String> { Err("网络请求失败".into()) };
        assert_eq!(search("crossref", "x", 5, &offline), Err("网络请求失败".into()));
        assert!(KINDS.iter().all(|(kind, _)| !syntax(kind).is_empty()));
    }
}
