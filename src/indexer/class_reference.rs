use lsp_types::{Location, Position, Range, Url};

use crate::{
    db::{ClassRecord, ClassRecordKind, Db},
    get_node_name,
};

use super::index;

const NODE_ID: u16 = 308;
#[derive(Debug, Default)]
pub struct ClassReferenceIndexer {}
impl index::Indexer for ClassReferenceIndexer {
    fn do_index(&self, index: &mut Db, document: &[u8], node: &tree_sitter::Node, url: &Url) {
        log::debug!("Start Index of References");
        // first we need to check if an object create with FQN
        // that is we need to look for qualified_name(ID=207) child
        // if there is not, look for name(ID=1)
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind_id() == 207 {
                    log::debug!(
                        "I found qualifiued name child for you {:?} {:?} {:#?}, {:?}",
                        child.kind_id(),
                        child.id(),
                        child,
                        child.utf8_text(document)
                    );
                    let mut prefix = None;
                    let mut name = None;
                    for i in 0..child.child_count() {
                        if let Some(n) = child.child(i) {
                            if n.kind_id() == 208 {
                                prefix = n.utf8_text(document).ok();
                            }
                            if n.kind_id() == 1 {
                                name = n.utf8_text(document).ok();
                            }
                        }
                    }
                    log::debug!("--->>>PREFIX {:?}", prefix);
                    log::debug!("--->>>NAME {:?}", name);
                }
            }
        }

        //let class_name = get_node_name(&node, document).unwrap_or(String::new());
        //let mut ns_name = String::new();
        //let mut sibling = node.prev_sibling();
        //while let Some(s) = sibling {
        //    //log::debug!("{s:?} => {:?}", s.kind_id());
        //    // namespace_definition == 204
        //    if s.kind_id() == 308 {
        //        ns_name = get_node_name(&s, document).unwrap_or(String::new());
        //        break;
        //    }
        //    sibling = s.prev_sibling();
        //}
        //log::debug!("Full qualified name of class {}\\{}", ns_name, class_name);
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
        //let record = ClassRecord {
        //    id: 0,
        //    kind: ClassRecordKind::Class,
        //    fqn: format!("{}\\{}", ns_name, &class_name),
        //    name: class_name,
        //    implementations: Vec::new(),
        //    implements: Vec::new(),
        //    location,
        //};
        //let r = index.save_row(&record).expect("Save record");
        //log::debug!("Result of saving record {:?}", r);
    }
    fn can_index(&self, node: &tree_sitter::Node) -> bool {
        node.kind_id() == NODE_ID
    }
}
impl ClassReferenceIndexer {}
