use lsp_types::{Location, Position, Range, Url};

use crate::{
    db::{ClassRecord, ClassRecordKind, Db},
    get_node_name,
};

use super::index;

const NODE_ID: u16 = 204;
#[derive(Debug, Default)]
pub struct ClassDeclarationIndexer {}
impl index::Indexer for ClassDeclarationIndexer {
    fn do_index(&self, index: &mut Db, document: &[u8], node: &tree_sitter::Node, url: &Url) {
        let class_name = get_node_name(&node, document).unwrap_or(String::new());
        let mut ns_name = String::new();
        let mut sibling = node.prev_sibling();
        while let Some(s) = sibling {
            //log::debug!("{s:?} => {:?}", s.kind_id());
            // namespace_definition == 204
            if s.kind_id() == 204 {
                ns_name = get_node_name(&s, document).unwrap_or(String::new());
                break;
            }
            sibling = s.prev_sibling();
        }
        log::debug!("Full qualified name of class {}\\{}", ns_name, class_name);
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                match child.kind() {
                    "class_interface_clause" => self.index_class_interfaces(index, document, node),
                    "base_clause" => self.index_base_class(index, document, node),
                    "attribute_list" => (),
                    _ => continue,
                }
            }
        }
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
        let record = ClassRecord {
            id: 0,
            kind: ClassRecordKind::Class,
            fqn: format!("{}\\{}", ns_name, &class_name),
            name: class_name,
            implementations: Vec::new(),
            implements: Vec::new(),
            location,
        };
        let r = index.save_row(&record).expect("Save record");
        log::debug!("Result of saving record {:?}", r);
    }
    fn can_index(&self, node: &tree_sitter::Node) -> bool {
        node.kind_id() == NODE_ID
    }
}
impl ClassDeclarationIndexer {
    pub fn index_class_interfaces(
        &self,
        index: &mut impl index::Index,
        document: &[u8],
        node: &tree_sitter::Node,
    ) {
        if let Some(child) = node.child_by_field_name("class_interface_clause") {
            log::debug!("{child:?}")
        }
    }
    pub fn index_base_class(
        &self,
        index: &mut impl index::Index,
        document: &[u8],
        node: &tree_sitter::Node,
    ) {
    }
}
