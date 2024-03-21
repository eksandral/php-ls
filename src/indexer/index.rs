use lsp_types::Url;
use tree_sitter::Tree;

use crate::db::Db;

pub trait Indexer {
    fn index(&self, index: &mut Db, document: &[u8], tree: &Tree, url: &Url) -> anyhow::Result<()>;
}

pub trait Index {
    type Record;
    type Err;
    type SaveResult;
    fn save(&mut self, row: &Self::Record) -> Result<Self::SaveResult, Self::Err>;
}
pub trait TextDocument {
    fn as_bytes(&self) -> &[u8];
}
impl TextDocument for &[u8] {
    fn as_bytes(&self) -> &[u8] {
        self
    }
}
