use lsp_types::{Location, Position, Range, Url};
use tree_sitter::{Query, QueryCursor};
use tree_sitter_php::language_php;

use crate::{
    db::{ClassRecord, ClassRecordKind, Db},
    get_node_name, ToLocation,
};

use super::index;

const NODE_ID: &'static str = "class_declaration";
#[derive(Debug, Default)]
pub struct ClassDeclarationIndexer {}
impl index::Indexer for ClassDeclarationIndexer {
    fn index(
        &self,
        index: &mut Db,
        document: &[u8],
        tree: &tree_sitter::Tree,
        url: &Url,
    ) -> anyhow::Result<()> {
        let root_node = tree.root_node();
        let queries = vec![
            // Namespace detection
            vec!["(namespace_definition (namespace_name) @ns_name)"],
            vec!["(class_declaration (name) @class_name)"],
            vec![
                "(declaration_list (method_declaration
               name: (name) @method_name
               parameters: (formal_parameters) @params
               return_type: (_)? @return_type
))",
            ],
        ];
        let mut current_namespace = "";
        let mut current_classname = "";
        for (idx, query) in queries.iter().enumerate() {
            let query = query.join("\n");
            let query = Query::new(language_php(), &query)?;
            let mut query_cursor = QueryCursor::new();
            let matches = query_cursor.matches(&query, root_node, &document[..]);
            let mut comment = "";
            for m in matches {
                //log::debug!("current idx = {}", idx);
                //log::debug!("Captures {:#?}", m.captures);
                match idx {
                    0 => {
                        current_namespace = m.captures[0].node.utf8_text(&document).unwrap_or("");
                    }
                    1 => {
                        let comment = m.captures[0]
                            .node
                            // look for parent node which should be method_declaration
                            .parent()
                            .map(|p| {
                                // check if there is a prev sibling that has comment type
                                p.prev_sibling()
                                    .filter(|c| c.kind() == "comment")
                                    .map(|c| c.utf8_text(&document).ok())
                                    .flatten()
                            })
                            .flatten()
                            .unwrap_or_default();
                        let desc: Vec<&str> = comment
                            .split("\n")
                            .map(|x| x.trim())
                            .filter(|&x| {
                                !x.starts_with("* @")
                                    && !x.starts_with("/**")
                                    && !x.starts_with("*/")
                            })
                            .map(|x| if x.len() > 1 { x[1..].trim() } else { "" })
                            .collect();
                        let description = desc.join("\n");
                        current_classname = m.captures[0].node.utf8_text(&document).unwrap_or("");
                        let fqn = format!("{}\\{}", current_namespace, current_classname);

                        let record = ClassRecord {
                            id: 0,
                            fqn,
                            description,
                            location: m.captures[0].node.range().to_locaton(url),
                            parameters: None,
                            attributes: None,
                            return_type: None,
                        };
                        let r = index.save_row(&record).expect("Save record");
                    }
                    2 => {
                        //method declaration

                        let comment = m.captures[0]
                            .node
                            // look for parent node which should be method_declaration
                            .parent()
                            .map(|p| {
                                // check if there is a prev sibling that has comment type
                                p.prev_sibling()
                                    .filter(|c| c.kind() == "comment")
                                    .map(|c| c.utf8_text(&document).ok())
                                    .flatten()
                            })
                            .flatten()
                            .unwrap_or_default();
                        let desc: Vec<&str> = comment
                            .split("\n")
                            .map(|x| x.trim())
                            .filter(|&x| {
                                !x.starts_with("* @")
                                    && !x.starts_with("/**")
                                    && !x.starts_with("*/")
                            })
                            .map(|x| if x.len() > 1 { x[1..].trim() } else { "" })
                            .collect();
                        let comment = desc.join("\n");
                        let method_name = m.captures[0].node.utf8_text(&document).ok().unwrap();
                        let method_params = m.captures[1].node.utf8_text(&document).ok().unwrap();
                        let fqn = format!(
                            "{}\\{}::{}",
                            current_namespace, current_classname, method_name,
                        );
                        log::debug!("method's FQN = {}", fqn);
                        let return_type = m
                            .captures
                            .get(2)
                            .map(|rt| rt.node.utf8_text(&document).ok().map(|x| x.to_string()))
                            .flatten();

                        let record = ClassRecord {
                            id: 0,
                            fqn,
                            description: comment.to_string(),
                            location: m.captures[0].node.range().to_locaton(url),
                            parameters: Some(method_params.into()),
                            attributes: None,
                            return_type,
                        };
                        // @TODO handle error
                        let _ = index.save_row(&record).expect("Save record");
                    }
                    _ => (),
                }
            }
        }
        Ok(())
    }
}
