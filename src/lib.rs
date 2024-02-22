pub mod db;
pub mod indexer;

use std::str::FromStr;

use lsp_types::{Location, Position, Range, Url};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteQueryResult, SqliteRow},
    ConnectOptions, Executor, Row, SqliteConnection, SqlitePool,
};
use tree_sitter::Node;

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
    println!(
        "{:width$}{:?} => {:?}({}), ID={:?}",
        "",
        node,
        node.kind(),
        node.kind_id(),
        node.id()
    );
}
