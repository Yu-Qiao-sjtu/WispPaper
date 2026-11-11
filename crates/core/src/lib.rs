//! Wisp Paper's library engine. No GPUI in here: everything can be compiled
//! and tested on its own (`cargo test -p wisp_core`).
//!
//! - `model`    papers, custom field definitions, attachments, normalisation
//! - `libraries` the independent libraries under the data folder
//! - `store`    one library on disk: a SQLite file plus `papers/`
//! - `filter`   search, filters, sorting and the sidebar counts
//! - `import`   BibTeX and RIS readers
//! - `export`   BibTeX and CSV writers
//! - `lookup`   Crossref / Europe PMC records → `Paper`
//! - `fulltext` page text of a stored PDF
//! - `ai`       context building, request bodies and stream parsing for 文献库提问
//! - `settings` settings.json

pub mod ai;
pub mod export;
pub mod filter;
pub mod fulltext;
pub mod import;
pub mod libraries;
pub mod lookup;
pub mod model;
pub mod settings;
pub mod store;
pub mod time;
