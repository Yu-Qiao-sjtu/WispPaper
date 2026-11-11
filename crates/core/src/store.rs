//! The library on disk: `wisp-paper.sqlite` plus the attachments under
//! `papers/`. Paths in the database are relative to the library folder, so
//! the whole folder can be copied elsewhere and keeps working.

use crate::model::*;
use crate::time::now_iso;
use anyhow::{Context, Result, anyhow, bail};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const DB_FILE: &str = "wisp-paper.sqlite";
/// Bump when an older build would misread the database.
pub const SCHEMA_VERSION: i32 = 1;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS papers (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    -- Lowercase letters and digits of the title: finds duplicates without identifiers.
    -- Not unique: the same title with another DOI is another paper (preprint and published version).
    title_key TEXT NOT NULL DEFAULT '',
    abstract TEXT NOT NULL DEFAULT '',
    authors_json TEXT NOT NULL DEFAULT '[]',
    journal TEXT NOT NULL DEFAULT '',
    year INTEGER CHECK (year IS NULL OR year BETWEEN 1800 AND 2200),
    publication_state TEXT NOT NULL DEFAULT 'unknown'
        CHECK (publication_state IN ('unknown','preprint','published','unpublished')),
    url TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT '',
    screening_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (screening_status IN ('pending','included','excluded','uncertain','irrelevant')),
    screening_reason TEXT NOT NULL DEFAULT '',
    reviewed_at TEXT NOT NULL DEFAULT '',
    reading_status TEXT NOT NULL DEFAULT 'unread' CHECK (reading_status IN ('unread','skimmed','read')),
    starred INTEGER NOT NULL DEFAULT 0 CHECK (starred IN (0,1)),
    tags_json TEXT NOT NULL DEFAULT '[]',
    notes TEXT NOT NULL DEFAULT '',
    first_seen_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_papers_title_key ON papers(title_key);

-- Bare identifiers: lowercase DOI without the doi.org prefix, PMID digits only.
CREATE TABLE IF NOT EXISTS paper_identifiers (
    paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    namespace TEXT NOT NULL CHECK (namespace IN ('doi','pmid')),
    value TEXT NOT NULL CHECK (length(value) > 0),
    PRIMARY KEY (namespace, value)
);
CREATE INDEX IF NOT EXISTS idx_identifiers_paper ON paper_identifiers(paper_id);

-- 自定义词条: the fields this library adds to every paper.
CREATE TABLE IF NOT EXISTS field_defs (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE CHECK (length(trim(name)) > 0),
    kind TEXT NOT NULL DEFAULT 'text' CHECK (kind IN ('text','tags','select')),
    options_json TEXT NOT NULL DEFAULT '[]',
    in_table INTEGER NOT NULL DEFAULT 0 CHECK (in_table IN (0,1)),
    position INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS paper_fields (
    paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    field_id TEXT NOT NULL REFERENCES field_defs(id) ON DELETE CASCADE,
    values_json TEXT NOT NULL DEFAULT '[]',
    PRIMARY KEY (paper_id, field_id)
);

-- Attachments. local_path is relative to the library folder, `/` separated.
CREATE TABLE IF NOT EXISTS resources (
    id TEXT PRIMARY KEY NOT NULL,
    paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    resource_type TEXT NOT NULL CHECK (resource_type IN ('fulltext','supplement','other')),
    title TEXT NOT NULL,
    format TEXT NOT NULL DEFAULT '',
    local_path TEXT NOT NULL,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    sha256 TEXT NOT NULL DEFAULT '',
    added_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_resources_paper ON resources(paper_id);

-- Page text of a stored PDF, extracted once: 文献库提问 and search read it.
CREATE TABLE IF NOT EXISTS fulltext_text (
    resource_id TEXT PRIMARY KEY NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
    pages TEXT NOT NULL DEFAULT '[]',
    error TEXT NOT NULL DEFAULT '',
    extracted_at TEXT NOT NULL
);

-- 回收站: the paper keeps its notes, fields and files until it is deleted for good.
CREATE TABLE IF NOT EXISTS trashed_papers (
    paper_id TEXT PRIMARY KEY NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    deleted_at TEXT NOT NULL
);
"#;

const PAPER_COLUMNS: &str = "id, title, abstract, authors_json, journal, year, publication_state, url, source, \
    screening_status, screening_reason, reviewed_at, reading_status, starred, tags_json, notes, first_seen_at, updated_at";

/// A record an import left out because the library already holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Skipped {
    pub title: String,
    pub existing_id: String,
    pub in_trash: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportResult {
    pub added: Vec<Paper>,
    pub skipped: Vec<Skipped>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrashEntry {
    pub paper: Paper,
    pub deleted_at: String,
}

pub struct Store {
    root: PathBuf,
    db: Connection,
}

fn json_strings(value: &str) -> Vec<String> {
    serde_json::from_str(value).unwrap_or_default()
}

fn to_json(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".into())
}

impl Store {
    /// Open (creating it when new) the library in `root`.
    pub fn open(root: &Path) -> Result<Store> {
        std::fs::create_dir_all(root.join("papers")).with_context(|| format!("无法创建文献库文件夹 {}", root.display()))?;
        let db = Connection::open(root.join(DB_FILE)).with_context(|| format!("无法打开数据库 {}", root.join(DB_FILE).display()))?;
        Self::init(root.to_path_buf(), db)
    }

    /// A library that lives only in memory; attachments go under `root`.
    pub fn in_memory(root: &Path) -> Result<Store> {
        Self::init(root.to_path_buf(), Connection::open_in_memory()?)
    }

    fn init(root: PathBuf, db: Connection) -> Result<Store> {
        db.pragma_update(None, "foreign_keys", "ON")?;
        // WAL keeps a crash from losing the library; ignored for in-memory databases.
        let _ = db.pragma_update(None, "journal_mode", "WAL");
        let version: i32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            bail!("这个文献库由更新版本的 Wisp Paper 创建（数据库版本 {version}），请升级后再打开。");
        }
        db.execute_batch(SCHEMA)?;
        db.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Store { root, db })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// How many papers the database at `path` holds outside the trash,
    /// without creating or changing anything; 0 when it cannot be read.
    pub fn count_papers(path: &Path) -> usize {
        let Ok(db) = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) else { return 0 };
        db.query_row("SELECT count(*) FROM papers WHERE id NOT IN (SELECT paper_id FROM trashed_papers)", [], |r| r.get::<_, i64>(0)).map(|n| n as usize).unwrap_or(0)
    }

    // ---- papers

    /// Every paper outside the trash, newest first.
    pub fn list_papers(&self) -> Result<Vec<Paper>> {
        self.read_papers("WHERE id NOT IN (SELECT paper_id FROM trashed_papers) ORDER BY first_seen_at DESC, id")
    }

    pub fn get_paper(&self, id: &str) -> Result<Paper> {
        let mut stmt = self.db.prepare(&format!("SELECT {PAPER_COLUMNS} FROM papers WHERE id = ?1"))?;
        let mut paper = stmt.query_row([id], row_to_paper).optional()?.ok_or_else(|| anyhow!("文献不存在：{id}"))?;
        self.attach_details(std::slice::from_mut(&mut paper))?;
        Ok(paper)
    }

    fn read_papers(&self, tail: &str) -> Result<Vec<Paper>> {
        let mut stmt = self.db.prepare(&format!("SELECT {PAPER_COLUMNS} FROM papers {tail}"))?;
        let mut papers = stmt.query_map([], row_to_paper)?.collect::<rusqlite::Result<Vec<_>>>()?;
        self.attach_details(&mut papers)?;
        Ok(papers)
    }

    /// Identifiers, custom field values and the full-text status live in other tables.
    fn attach_details(&self, papers: &mut [Paper]) -> Result<()> {
        let index: HashMap<String, usize> = papers.iter().enumerate().map(|(i, p)| (p.id.clone(), i)).collect();
        let mut stmt = self.db.prepare("SELECT paper_id, namespace, value FROM paper_identifiers")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let (id, namespace, value): (String, String, String) = (row.get(0)?, row.get(1)?, row.get(2)?);
            if let Some(&i) = index.get(&id) {
                if namespace == "doi" { papers[i].doi = value } else { papers[i].pmid = value }
            }
        }
        let mut stmt = self.db.prepare("SELECT paper_id, field_id, values_json FROM paper_fields")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let (id, field, values): (String, String, String) = (row.get(0)?, row.get(1)?, row.get(2)?);
            let values = json_strings(&values);
            if let Some(&i) = index.get(&id) && !values.is_empty() {
                papers[i].fields.insert(field, values);
            }
        }
        let mut stmt = self.db.prepare("SELECT DISTINCT paper_id FROM resources WHERE resource_type = 'fulltext'")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            if let Some(&i) = index.get(&id) {
                papers[i].fulltext = "stored".into();
            }
        }
        for paper in papers.iter_mut() {
            if paper.fulltext != "stored" {
                paper.fulltext = if paper.link().is_empty() { "missing" } else { "linked" }.into();
            }
        }
        Ok(())
    }

    /// The paper the library already holds for `p`: same DOI, same PMID, or,
    /// when one side has no DOI, the same title. Papers in the trash count.
    pub fn existing_paper(&self, p: &Paper) -> Result<Option<String>> {
        existing_paper(&self.db, p)
    }

    /// Add one paper; an error names the duplicate when the library holds it.
    pub fn add_paper(&mut self, paper: Paper) -> Result<Paper> {
        let title = paper.title.clone();
        let mut result = self.import_papers(vec![paper])?;
        match result.added.pop() {
            Some(added) => Ok(added),
            None => {
                let skipped = result.skipped.pop().map(|s| if s.in_trash { "在回收站中" } else { "已在文献库中" }).unwrap_or("已存在");
                bail!("「{title}」{skipped}。")
            }
        }
    }

    /// Insert a batch in one transaction: an invalid record rolls everything
    /// back, a known paper is skipped and reported, so rerunning an import is safe.
    pub fn import_papers(&mut self, papers: Vec<Paper>) -> Result<ImportResult> {
        let tx = self.db.transaction()?;
        let mut result = ImportResult::default();
        for (i, mut paper) in papers.into_iter().enumerate() {
            paper.normalize();
            paper.validate().map_err(|e| anyhow!("第 {} 条：{e}", i + 1))?;
            if let Some(existing) = existing_paper(&tx, &paper)? {
                let in_trash = tx.query_row("SELECT 1 FROM trashed_papers WHERE paper_id = ?1", [&existing], |_| Ok(())).optional()?.is_some();
                result.skipped.push(Skipped { title: paper.title, existing_id: existing, in_trash });
                continue;
            }
            if paper.id.is_empty() || tx.query_row("SELECT 1 FROM papers WHERE id = ?1", [&paper.id], |_| Ok(())).optional()?.is_some() {
                paper.id = new_id("P");
            }
            write_paper(&tx, &paper)?;
            result.added.push(paper);
        }
        tx.commit()?;
        Ok(result)
    }

    /// Save an edited paper. The identifiers must not belong to another paper.
    pub fn save_paper(&mut self, paper: &mut Paper) -> Result<()> {
        paper.normalize();
        paper.validate().map_err(|e| anyhow!(e))?;
        let tx = self.db.transaction()?;
        for (namespace, value) in [("doi", &paper.doi), ("pmid", &paper.pmid)] {
            if value.is_empty() { continue }
            let other: Option<String> = tx.query_row("SELECT paper_id FROM paper_identifiers WHERE namespace = ?1 AND value = ?2 AND paper_id <> ?3", params![namespace, value, paper.id], |r| r.get(0)).optional()?;
            if let Some(other) = other {
                let title: String = tx.query_row("SELECT title FROM papers WHERE id = ?1", [&other], |r| r.get(0))?;
                bail!("{} {value} 已属于另一篇文献：「{title}」。", namespace.to_uppercase());
            }
        }
        write_paper(&tx, paper)?;
        tx.commit()?;
        Ok(())
    }

    // ---- trash

    pub fn trash_paper(&mut self, id: &str) -> Result<()> {
        self.db.execute("INSERT OR IGNORE INTO trashed_papers(paper_id, deleted_at) VALUES (?1, ?2)", params![id, now_iso()])?;
        Ok(())
    }

    pub fn restore_paper(&mut self, id: &str) -> Result<()> {
        self.db.execute("DELETE FROM trashed_papers WHERE paper_id = ?1", [id])?;
        Ok(())
    }

    pub fn list_trash(&self) -> Result<Vec<TrashEntry>> {
        let papers = self.read_papers("WHERE id IN (SELECT paper_id FROM trashed_papers) ORDER BY id")?;
        let mut entries = vec![];
        for paper in papers {
            let deleted_at: String = self.db.query_row("SELECT deleted_at FROM trashed_papers WHERE paper_id = ?1", [&paper.id], |r| r.get(0))?;
            entries.push(TrashEntry { paper, deleted_at });
        }
        entries.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at));
        Ok(entries)
    }

    /// Remove a paper and its records for good. Its files move to `removed/`
    /// inside the library folder rather than being unlinked.
    pub fn delete_paper(&mut self, id: &str) -> Result<()> {
        let files = self.paper_files(id)?;
        self.db.execute("DELETE FROM papers WHERE id = ?1", [id])?;
        for file in files {
            self.set_aside(&file.local_path);
        }
        Ok(())
    }

    // ---- custom fields

    pub fn list_fields(&self) -> Result<Vec<FieldDef>> {
        let mut stmt = self.db.prepare("SELECT id, name, kind, options_json, in_table FROM field_defs ORDER BY position, name")?;
        let fields = stmt
            .query_map([], |r| {
                Ok(FieldDef { id: r.get(0)?, name: r.get(1)?, kind: FieldKind::parse(&r.get::<_, String>(2)?), options: json_strings(&r.get::<_, String>(3)?), in_table: r.get::<_, i32>(4)? == 1 })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(fields)
    }

    /// Create (empty id) or update a field definition.
    pub fn save_field(&mut self, field: &mut FieldDef) -> Result<()> {
        field.name = field.name.trim().to_string();
        field.options = clean_strings(field.options.iter());
        if field.name.is_empty() { bail!("请填写词条名称。") }
        if field.name.chars().count() > 40 { bail!("词条名称不能超过 40 个字符。") }
        if field.kind == FieldKind::Select && field.options.is_empty() { bail!("单选词条至少需要一个选项。") }
        let clash: Option<String> = self.db.query_row("SELECT id FROM field_defs WHERE name = ?1 AND id <> ?2", params![field.name, field.id], |r| r.get(0)).optional()?;
        if clash.is_some() { bail!("已有同名词条「{}」。", field.name) }
        if field.id.is_empty() {
            field.id = new_id("F");
            let position: i64 = self.db.query_row("SELECT coalesce(max(position), 0) + 1 FROM field_defs", [], |r| r.get(0))?;
            self.db.execute(
                "INSERT INTO field_defs(id, name, kind, options_json, in_table, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![field.id, field.name, field.kind.as_str(), to_json(&field.options), field.in_table as i32, position],
            )?;
        } else {
            self.db.execute(
                "UPDATE field_defs SET name = ?2, kind = ?3, options_json = ?4, in_table = ?5 WHERE id = ?1",
                params![field.id, field.name, field.kind.as_str(), to_json(&field.options), field.in_table as i32],
            )?;
        }
        Ok(())
    }

    /// Delete a field and every value papers hold for it.
    pub fn delete_field(&mut self, id: &str) -> Result<()> {
        self.db.execute("DELETE FROM field_defs WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Move a field one place up (`-1`) or down (`1`) in the order.
    pub fn move_field(&mut self, id: &str, step: i32) -> Result<()> {
        let mut ids: Vec<String> = self.list_fields()?.into_iter().map(|f| f.id).collect();
        let Some(at) = ids.iter().position(|f| f == id) else { return Ok(()) };
        let to = at as i32 + step;
        if to < 0 || to as usize >= ids.len() { return Ok(()) }
        ids.swap(at, to as usize);
        let tx = self.db.transaction()?;
        for (position, id) in ids.iter().enumerate() {
            tx.execute("UPDATE field_defs SET position = ?2 WHERE id = ?1", params![id, position as i64])?;
        }
        tx.commit()?;
        Ok(())
    }

    // ---- files

    pub fn list_files(&self) -> Result<Vec<PaperFile>> {
        self.read_files("", [])
    }

    pub fn paper_files(&self, paper_id: &str) -> Result<Vec<PaperFile>> {
        self.read_files("WHERE paper_id = ?1", [paper_id])
    }

    fn read_files<P: rusqlite::Params>(&self, filter: &str, args: P) -> Result<Vec<PaperFile>> {
        let mut stmt = self.db.prepare(&format!("SELECT id, paper_id, resource_type, title, format, local_path, size_bytes, added_at FROM resources {filter} ORDER BY resource_type, added_at"))?;
        let files = stmt
            .query_map(args, |r| {
                Ok(PaperFile { id: r.get(0)?, paper_id: r.get(1)?, kind: r.get(2)?, title: r.get(3)?, format: r.get(4)?, local_path: r.get(5)?, size_bytes: r.get::<_, i64>(6)? as u64, added_at: r.get(7)? })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(files)
    }

    /// Copy `source` into `papers/` and attach it to the paper. The original
    /// stays where it is. The stored name is the resource id, so odd file
    /// names survive a move between systems; the same content attached to
    /// the same paper twice is stored once.
    pub fn import_file(&mut self, paper_id: &str, source: &Path, kind: &str) -> Result<PaperFile> {
        if !["fulltext", "supplement", "other"].contains(&kind) { bail!("不支持的附件类型：{kind}") }
        self.db.query_row("SELECT 1 FROM papers WHERE id = ?1", [paper_id], |_| Ok(())).optional()?.ok_or_else(|| anyhow!("文献不存在：{paper_id}"))?;
        let title = source.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "附件".into());
        let format: String = source.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default().chars().filter(|c| c.is_ascii_alphanumeric()).take(10).collect();
        let id = new_id("R");
        let relative = if format.is_empty() { format!("papers/{id}") } else { format!("papers/{id}.{format}") };
        let target = self.root.join(&relative);
        std::fs::create_dir_all(self.root.join("papers"))?;
        let (size, sha) = copy_hashed(source, &target).with_context(|| format!("无法复制文件 {}", source.display()))?;
        let same: Option<String> = self.db.query_row("SELECT id FROM resources WHERE paper_id = ?1 AND sha256 = ?2", params![paper_id, sha], |r| r.get(0)).optional()?;
        if let Some(same) = same {
            let _ = std::fs::remove_file(&target);
            return self.read_files("WHERE id = ?1", [same])?.pop().ok_or_else(|| anyhow!("附件记录丢失"));
        }
        let file = PaperFile { id, paper_id: paper_id.to_string(), kind: kind.to_string(), title, format, local_path: relative, size_bytes: size, added_at: now_iso() };
        let inserted = self.db.execute(
            "INSERT INTO resources(id, paper_id, resource_type, title, format, local_path, size_bytes, sha256, added_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![file.id, file.paper_id, file.kind, file.title, file.format, file.local_path, file.size_bytes as i64, sha, file.added_at],
        );
        if let Err(err) = inserted {
            let _ = std::fs::remove_file(&target);
            return Err(err.into());
        }
        Ok(file)
    }

    /// The absolute path of a stored file.
    pub fn file_path(&self, file: &PaperFile) -> PathBuf {
        self.root.join(file.local_path.replace('/', std::path::MAIN_SEPARATOR_STR))
    }

    /// Detach a file. It moves to `removed/` inside the library folder.
    pub fn remove_file(&mut self, id: &str) -> Result<()> {
        let path: Option<String> = self.db.query_row("SELECT local_path FROM resources WHERE id = ?1", [id], |r| r.get(0)).optional()?;
        self.db.execute("DELETE FROM resources WHERE id = ?1", [id])?;
        if let Some(path) = path {
            self.set_aside(&path);
        }
        Ok(())
    }

    fn set_aside(&self, relative: &str) {
        let from = self.root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        let Some(name) = from.file_name() else { return };
        let dir = self.root.join("removed");
        if std::fs::create_dir_all(&dir).is_ok() {
            let _ = std::fs::rename(&from, dir.join(name));
        }
    }

    // ---- full text

    /// The cached page text of a file: `None` until it has been extracted,
    /// `Some(Err)` when extraction failed.
    pub fn fulltext_pages(&self, resource_id: &str) -> Result<Option<std::result::Result<Vec<String>, String>>> {
        let row: Option<(String, String)> = self.db.query_row("SELECT pages, error FROM fulltext_text WHERE resource_id = ?1", [resource_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        Ok(row.map(|(pages, error)| if error.is_empty() { Ok(json_strings(&pages)) } else { Err(error) }))
    }

    pub fn save_fulltext_pages(&mut self, resource_id: &str, pages: &std::result::Result<Vec<String>, String>) -> Result<()> {
        let (pages, error) = match pages {
            Ok(pages) => (to_json(pages), String::new()),
            Err(error) => ("[]".to_string(), error.clone()),
        };
        // The file may have been removed while its text was being extracted.
        self.db.execute(
            "INSERT INTO fulltext_text(resource_id, pages, error, extracted_at) SELECT ?1, ?2, ?3, ?4 WHERE EXISTS (SELECT 1 FROM resources WHERE id = ?1) \
             ON CONFLICT(resource_id) DO UPDATE SET pages = excluded.pages, error = excluded.error, extracted_at = excluded.extracted_at",
            params![resource_id, pages, error, now_iso()],
        )?;
        Ok(())
    }

    /// Every extracted full text, by paper id: (file, pages).
    pub fn all_fulltext(&self) -> Result<BTreeMap<String, (PaperFile, Vec<String>)>> {
        let mut out = BTreeMap::new();
        for file in self.read_files("WHERE resource_type = 'fulltext' AND paper_id NOT IN (SELECT paper_id FROM trashed_papers)", [])? {
            if out.contains_key(&file.paper_id) { continue }
            if let Some(Ok(pages)) = self.fulltext_pages(&file.id)? {
                out.insert(file.paper_id.clone(), (file, pages));
            }
        }
        Ok(out)
    }
}

fn row_to_paper(r: &rusqlite::Row) -> rusqlite::Result<Paper> {
    Ok(Paper {
        id: r.get(0)?,
        title: r.get(1)?,
        abstract_text: r.get(2)?,
        authors: json_strings(&r.get::<_, String>(3)?).join("; "),
        journal: r.get(4)?,
        year: r.get(5)?,
        publication_status: r.get(6)?,
        url: r.get(7)?,
        source: r.get(8)?,
        screening: r.get(9)?,
        screening_reason: r.get(10)?,
        reviewed_at: r.get(11)?,
        reading: r.get(12)?,
        starred: r.get::<_, i32>(13)? == 1,
        tags: json_strings(&r.get::<_, String>(14)?),
        notes: r.get(15)?,
        added_at: r.get(16)?,
        updated_at: r.get(17)?,
        ..Default::default()
    })
}

fn existing_paper(db: &Connection, p: &Paper) -> Result<Option<String>> {
    for (namespace, value) in [("doi", &p.doi), ("pmid", &p.pmid)] {
        if value.is_empty() { continue }
        let found: Option<String> = db.query_row("SELECT paper_id FROM paper_identifiers WHERE namespace = ?1 AND value = ?2", params![namespace, value], |r| r.get(0)).optional()?;
        if found.is_some() { return Ok(found) }
    }
    let key = title_key(&p.title);
    if key.is_empty() { return Ok(None) }
    // The same title under two DOIs is two papers; without a DOI on either side it is one.
    let found: Option<String> = db
        .query_row(
            "SELECT id FROM papers WHERE title_key = ?1 AND (?2 = '' OR NOT EXISTS (SELECT 1 FROM paper_identifiers i WHERE i.paper_id = papers.id AND i.namespace = 'doi')) LIMIT 1",
            params![key, p.doi],
            |r| r.get(0),
        )
        .optional()?;
    Ok(found)
}

fn write_paper(tx: &Transaction, p: &Paper) -> Result<()> {
    let authors: Vec<String> = p.authors.split(';').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
    tx.execute(
        "INSERT INTO papers(id, title, title_key, abstract, authors_json, journal, year, publication_state, url, source, \
            screening_status, screening_reason, reviewed_at, reading_status, starred, tags_json, notes, first_seen_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19) \
         ON CONFLICT(id) DO UPDATE SET title = excluded.title, title_key = excluded.title_key, abstract = excluded.abstract, \
            authors_json = excluded.authors_json, journal = excluded.journal, year = excluded.year, \
            publication_state = excluded.publication_state, url = excluded.url, source = excluded.source, \
            screening_status = excluded.screening_status, screening_reason = excluded.screening_reason, reviewed_at = excluded.reviewed_at, \
            reading_status = excluded.reading_status, starred = excluded.starred, tags_json = excluded.tags_json, notes = excluded.notes, \
            first_seen_at = excluded.first_seen_at, updated_at = excluded.updated_at",
        params![
            p.id, p.title, title_key(&p.title), p.abstract_text, to_json(&authors), p.journal, p.year, p.publication_status, p.url, p.source,
            p.screening, p.screening_reason, p.reviewed_at, p.reading, p.starred as i32, to_json(&p.tags), p.notes, p.added_at, p.updated_at
        ],
    )
    .with_context(|| format!("保存文献失败：{}", p.title))?;
    tx.execute("DELETE FROM paper_identifiers WHERE paper_id = ?1", [&p.id])?;
    for (namespace, value) in [("doi", &p.doi), ("pmid", &p.pmid)] {
        if !value.is_empty() {
            tx.execute("INSERT INTO paper_identifiers(paper_id, namespace, value) VALUES (?1, ?2, ?3)", params![p.id, namespace, value])?;
        }
    }
    tx.execute("DELETE FROM paper_fields WHERE paper_id = ?1", [&p.id])?;
    for (field, values) in &p.fields {
        // Values of a field that was deleted meanwhile are dropped.
        tx.execute(
            "INSERT INTO paper_fields(paper_id, field_id, values_json) SELECT ?1, ?2, ?3 WHERE EXISTS (SELECT 1 FROM field_defs WHERE id = ?2)",
            params![p.id, field, to_json(values)],
        )?;
    }
    Ok(())
}

fn copy_hashed(source: &Path, target: &Path) -> Result<(u64, String)> {
    let mut input = std::fs::File::open(source)?;
    let mut output = std::fs::File::create(target)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    let mut size = 0u64;
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 { break }
        hasher.update(&buffer[..n]);
        output.write_all(&buffer[..n])?;
        size += n as u64;
    }
    output.flush()?;
    Ok((size, hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wisp-test-{name}-{}", new_id("T")));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn paper(title: &str, doi: &str) -> Paper {
        Paper { title: title.into(), doi: doi.into(), authors: "Zhang Wei; O'Brien Ann".into(), year: Some(2024), ..Default::default() }
    }

    #[test]
    fn add_list_and_duplicates() {
        let dir = temp_dir("add");
        let mut store = Store::open(&dir).unwrap();
        let added = store.add_paper(paper("Single-cell atlas", "https://doi.org/10.1000/ABC")).unwrap();
        assert_eq!(added.doi, "10.1000/abc");
        // Same DOI, same title without a DOI: both known. Same title, other DOI: new.
        let result = store.import_papers(vec![paper("Other title", "10.1000/abc"), paper("single cell ATLAS.", ""), paper("Single-cell atlas", "10.1000/xyz")]).unwrap();
        assert_eq!(result.skipped.len(), 2);
        assert_eq!(result.added.len(), 1);
        let papers = store.list_papers().unwrap();
        assert_eq!(papers.len(), 2);
        assert_eq!(papers[0].authors, "Zhang Wei; O'Brien Ann");
        assert_eq!(papers[0].fulltext, "linked");
        assert!(store.add_paper(paper("", "")).is_err());
        drop(store);
        // Reopening reads the same library.
        assert_eq!(Store::open(&dir).unwrap().list_papers().unwrap().len(), 2);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn save_checks_identifiers() {
        let dir = temp_dir("save");
        let mut store = Store::in_memory(&dir).unwrap();
        let a = store.add_paper(paper("A paper", "10.1000/a")).unwrap();
        let mut b = store.add_paper(paper("B paper", "10.1000/b")).unwrap();
        b.doi = a.doi.clone();
        assert!(store.save_paper(&mut b).is_err());
        b.doi = "10.1000/b2".into();
        b.decide("included", "fits");
        b.tags = vec!["atlas".into()];
        store.save_paper(&mut b).unwrap();
        let read = store.get_paper(&b.id).unwrap();
        assert_eq!((read.doi.as_str(), read.screening.as_str(), read.tags.len()), ("10.1000/b2", "included", 1));
        assert!(!read.reviewed_at.is_empty());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn custom_fields() {
        let dir = temp_dir("fields");
        let mut store = Store::in_memory(&dir).unwrap();
        let mut species = FieldDef { name: "物种".into(), kind: FieldKind::Tags, in_table: true, ..Default::default() };
        store.save_field(&mut species).unwrap();
        let mut method = FieldDef { name: "方法".into(), kind: FieldKind::Select, options: vec!["scRNA-seq".into(), "snRNA-seq".into()], ..Default::default() };
        store.save_field(&mut method).unwrap();
        assert!(store.save_field(&mut FieldDef { name: "物种".into(), ..Default::default() }).is_err());
        assert!(store.save_field(&mut FieldDef { name: "空单选".into(), kind: FieldKind::Select, ..Default::default() }).is_err());

        let mut p = store.add_paper(paper("Rice roots", "")).unwrap();
        p.fields.insert(species.id.clone(), vec!["Oryza sativa".into(), "Oryza sativa".into()]);
        p.fields.insert(method.id.clone(), vec!["scRNA-seq".into()]);
        p.fields.insert("F-gone".into(), vec!["x".into()]);
        store.save_paper(&mut p).unwrap();
        let read = store.get_paper(&p.id).unwrap();
        assert_eq!(read.fields.len(), 2);
        assert_eq!(read.fields[&species.id], vec!["Oryza sativa"]);

        store.move_field(&method.id, -1).unwrap();
        assert_eq!(store.list_fields().unwrap()[0].name, "方法");
        store.delete_field(&species.id).unwrap();
        assert_eq!(store.get_paper(&p.id).unwrap().fields.len(), 1);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn trash_and_files() {
        let dir = temp_dir("files");
        let mut store = Store::open(&dir).unwrap();
        let p = store.add_paper(paper("With file", "")).unwrap();
        let source = dir.join("我的 论文.PDF");
        std::fs::write(&source, b"%PDF-1.4 fake").unwrap();
        let file = store.import_file(&p.id, &source, "fulltext").unwrap();
        assert_eq!((file.format.as_str(), file.title.as_str(), file.size_bytes), ("pdf", "我的 论文.PDF", 13));
        assert!(store.file_path(&file).exists());
        assert!(source.exists());
        // The same content again is the same attachment.
        assert_eq!(store.import_file(&p.id, &source, "fulltext").unwrap().id, file.id);
        assert_eq!(store.paper_files(&p.id).unwrap().len(), 1);
        assert_eq!(store.get_paper(&p.id).unwrap().fulltext, "stored");

        assert!(store.fulltext_pages(&file.id).unwrap().is_none());
        store.save_fulltext_pages(&file.id, &Ok(vec!["page one".into(), "page two".into()])).unwrap();
        assert_eq!(store.fulltext_pages(&file.id).unwrap().unwrap().unwrap().len(), 2);
        assert_eq!(store.all_fulltext().unwrap().len(), 1);

        store.trash_paper(&p.id).unwrap();
        assert!(store.list_papers().unwrap().is_empty());
        assert_eq!(store.list_trash().unwrap().len(), 1);
        // A trashed paper still blocks its re-import.
        assert!(store.import_papers(vec![paper("With file", "")]).unwrap().skipped[0].in_trash);
        store.restore_paper(&p.id).unwrap();
        assert_eq!(store.list_papers().unwrap().len(), 1);

        store.delete_paper(&p.id).unwrap();
        assert!(!store.file_path(&file).exists());
        assert!(dir.join("removed").read_dir().unwrap().count() == 1);
        assert!(store.list_files().unwrap().is_empty());
        std::fs::remove_dir_all(dir).ok();
    }
}
