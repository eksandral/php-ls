use lsp_types::{Location, Position, Range, Url};

use crate::{
    db::{ClassRecord, ClassRecordKind, Db},
    get_node_name,
};

use super::index;

const NODE_ID: u16 = 220;
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
                log::debug!("NS: {:?}", s.utf8_text(document));
                for i in 0..s.child_count() {
                    if let Some(child) = s.child(i) {
                        log::debug!("------ {child:?}");
                        if child.kind() == "namespace_name" {
                            ns_name = child
                                .utf8_text(document)
                                .ok()
                                .map_or("".to_string(), |x| x.to_string());
                            break;
                        }
                    }
                }
            }
            sibling = s.prev_sibling();
        }
        log::debug!("Full qualified name of class {}\\{}", ns_name, class_name);
        let class_fqn = format!("{ns_name}\\{class_name}");
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                log::debug!("CHILD OF CLASS {:?}", &child);
                match child.kind() {
                    "class_interface_clause" => self.index_class_interfaces(index, document, node),
                    "base_clause" => self.index_base_class(index, document, node),
                    "attribute_list" => (),
                    "declaration_list" => {
                        self.index_methods(index, document, &child, url, &class_fqn)
                    }
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
            fqn: class_fqn,
            location,
        };
        let r = index.save_row(&record).expect("Save record");
        log::debug!("Result of saving record {:?}", r);
    }
    fn can_index(&self, node: &tree_sitter::Node) -> bool {
        log::debug!("CAN I INDEX THIS NODE {node:?} {:?}", node.kind_id());
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
    fn index_methods(
        &self,
        index: &mut Db,
        document: &[u8],
        node: &tree_sitter::Node,
        url: &Url,
        class_fqn: &str,
    ) {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "method_declaration" {
                    'child: for n in 0..child.child_count() {
                        if let Some(child) = child.child(n) {
                            if child.kind() == "name" {
                                if let Some(name) = child.utf8_text(document).ok() {
                                    let range = child.range();
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
                                    let fqn = format!("{class_fqn}::{name}");
                                    let record = ClassRecord {
                                        id: 0,
                                        fqn,
                                        location,
                                    };
                                    let r = index.save_row(&record).expect("Save record");

                                    break 'child;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
