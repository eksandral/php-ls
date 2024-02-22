use lsp_types::Url;
use tree_sitter::Node;

use crate::db::Db;

pub trait Indexer {
    fn can_index(&self, node: &Node) -> bool;
    fn index(&self, index: &mut Db, document: &[u8], node: &Node, url: &Url) {
        if self.can_index(node) {
            self.do_index(index, document, node, url);
        }
    }
    fn do_index(&self, index: &mut Db, document: &[u8], node: &Node, url: &Url);
    fn before_parse(&self, index: &mut Db, document: &[u8]) {}
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
