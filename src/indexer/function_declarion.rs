use std::fmt::format;

use lsp_types::{Location, Position, Range, Url};

use crate::{
    db::{ClassRecord, ClassRecordKind, Db},
    get_node_name,
};

use super::index;

const NODE_ID: u16 = 234;
#[derive(Debug, Default)]
pub struct FunctionDeclarationIndexer {}
impl index::Indexer for FunctionDeclarationIndexer {
    fn do_index(&self, index: &mut Db, document: &[u8], node: &tree_sitter::Node, url: &Url) {
        log::debug!("Start Index of FUnctionDeclaration");
        let method_name = get_node_name(&node, document).unwrap_or(String::new());
        let range = node.range();
        let location: Location = Location {
            uri: url.clone(),
            range: Range {
                start: Position {
                    line: range.start_point.row as u32,
                    character: range.start_point.column as u32,
                },
                end: Position {
                    line: range.end_point.row as u32,
                    character: range.end_point.column as u32,
                },
            },
        };
        let class_row = index.get_class_by_method_location(&location).unwrap();

        let fqn = format!("{}::{}", class_row.fqn, method_name);
        let record = ClassRecord {
            id: 0,
            fqn,
            location,
        };
        index.save_row(&record).unwrap();
    }
    fn can_index(&self, node: &tree_sitter::Node) -> bool {
        node.kind_id() == NODE_ID
    }
}
