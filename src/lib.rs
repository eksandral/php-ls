pub mod db;
pub mod indexer;
pub mod utils;

use std::{cell::RefCell, str::FromStr};

use db::Db;
use lsp_types::{InitializeParams, Location, Position, Range, Url};
use sqlx::{sqlite::SqliteRow, Row};
use tree_sitter::Node;
thread_local! {

    pub static DB: RefCell<Option<Db>> = RefCell::new(None);
}

#[derive(Debug)]
pub struct Symbol {
    pub kind: SymbolKind,
    pub name: String,
    pub location: Location,
}

impl sqlx::FromRow<'_, SqliteRow> for Symbol {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        let uri: &str = row.try_get::<'_, &str, &str>("location_uri")?;
        let uri: Url = Url::from_str(uri).unwrap();
        let pos_start = Position::new(
            row.try_get("location_position_start_line")?,
            row.try_get("location_position_start_character")?,
        );
        let pos_end = Position::new(
            row.try_get("location_position_end_line")?,
            row.try_get("location_position_end_character")?,
        );
        let range = Range::new(pos_start, pos_end);
        let location: Location = Location::new(uri, range);

        Ok(Self {
            kind: Default::default(),
            name: row.try_get("name")?,
            location,
        })
    }
}

#[derive(Debug, Default)]
#[repr(u8)]
pub enum SymbolKind {
    #[default]
    Global = 0,
    Class,
    Method,
    Function,
    Variable,
}
pub fn get_node_name(node: &Node, document: &[u8]) -> Option<String> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind_id() == 1 {
                return child.utf8_text(document).ok().map(|x| x.to_string());
            }
        }
    }
    None
}
pub fn debug_node<'a>(node: &'a tree_sitter::Node<'a>, lvl: usize) {
    let width = lvl * 4;
    log::debug!(
        "{:width$}{:?} => {:?}({}), ID={:?}",
        "",
        node,
        node.kind(),
        node.kind_id(),
        node.id()
    );
}

pub trait ParamsGetProjectPath {
    fn get_project_path(&self) -> anyhow::Result<std::path::PathBuf>;
}
impl ParamsGetProjectPath for InitializeParams {
    fn get_project_path(&self) -> anyhow::Result<std::path::PathBuf> {
        let root_path = self
            .root_uri
            .clone()
            .map(|url| url.to_file_path().ok())
            .flatten()
            .ok_or(anyhow::anyhow!("Invalid project ROOT URI"))?;

        Ok(root_path)
    }
}
