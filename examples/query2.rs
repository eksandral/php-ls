use core::panic;
use std::{collections::HashMap, fmt::format, fs::read, os::fd::AsRawFd};

use tree_sitter::{Parser, Query, QueryCursor, QueryMatch};
use tree_sitter_php::language_php;

fn main() -> anyhow::Result<()> {
    //let path = "/home/eksandral/projects/php-template/singletone.php";
    let path = "/home/eksandral/projects/php-template/src/Engine.php";
    let _ = dotenv::dotenv().ok();
    env_logger::init();
    let mut parser = Parser::new();
    parser
        .set_language(tree_sitter_php::language_php())
        .expect("Error loading PHP parsing support");
    let contents = read(&path)?;
    let parsed = parser.parse(&contents, None);
    let tree = if let Some(tree) = parsed {
        tree
    } else {
        log::error!("I cannot parse {:?}", &path);
        panic!("Error");
    };
    let root_node = tree.root_node();
    print_tree(&root_node, 0);
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
        ], //vec!["(namespace_use_declaration (namespace_use_clause (qualified_name(namespace_name_as_prefix) @ns_q_name (name) @class_name) (namespace_aliasing_clause (name) @alias)?))",],
           ////
           //vec!["(assignment_expression ( (variable_name (name) @var_name)  (object_creation_expression (name) @object_name)))"],
           //vec!["(member_call_expression (variable_name) @varname (name) @method_name)",
           // "(member_call_expression (parenthesized_expression (object_creation_expression (name) @class_name) ) (name) @method_name)",
           //         "
           //   (member_call_expression
           //       (parenthesized_expression
           //           (object_creation_expression
           //               (qualified_name
           //                   (namespace_name_as_prefix)? @ns_as_prefix
           //                   (name) @cass_name
           //               )
           //           )
           //       )
           //       (name) @method_name
           //   )",
           //           "(scoped_call_expression (name) @class_name (name) @method_name)"
           //       ],
           //vec!["(obect_creation_expression (name)  @class_name)",
           //     "(obect_creation_expression (qualified_name (namespace_as_prefix) @ns_name (name) @class_name)"],
    ];
    //let mut ns_map = HashMap::new();
    //let mut vars_map = HashMap::new();

    let mut current_namespace = Some("");
    let mut current_classname = Some("");
    for (idx, query) in queries.iter().enumerate() {
        let query = query.join("\n");
        let query = Query::new(language_php(), &query)?;
        let mut query_cursor = QueryCursor::new();
        let matches = query_cursor.matches(&query, root_node, &contents[..]);
        let mut comment = Some("");
        for m in matches {
            //log::debug!("current idx = {}", idx);
            log::debug!("Captures {:#?}", m.captures);
            match idx {
                0 => {
                    current_namespace = m.captures[0].node.utf8_text(&contents).ok();
                }
                1 => {
                    current_classname = m.captures[0].node.utf8_text(&contents).ok();
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
                                .map(|c| c.utf8_text(&contents).ok())
                                .flatten()
                        })
                        .flatten()
                        .unwrap_or_default();
                    let desc: Vec<&str> = comment
                        .split("\n")
                        .map(|x| x.trim())
                        .filter(
                            |&x| x.len() > 2 && (&x[..3] != "* @" && &x[..3] != "/**"), //x[]("/**") || !x.starts_with("* @") || x[0..3] != "*/"
                        )
                        .map(|x| &x[2..])
                        .collect();
                    let comment = desc.join("\n");
                    let method_name = m.captures[0].node.utf8_text(&contents).ok().unwrap();
                    let method_params = m.captures[1].node.utf8_text(&contents).ok().unwrap();
                    let fqn = format!(
                        "{}\\{}::{}{}",
                        current_namespace.unwrap(),
                        current_classname.unwrap(),
                        method_name,
                        method_params,
                    );
                    log::debug!("method's FQN = {}", fqn);
                    let return_type = if let Some(rt) = m.captures.get(2) {
                        rt.node.utf8_text(&contents).ok()
                    } else {
                        Some("void")
                    };
                    log::debug!("Return type is {:?}", return_type);
                    log::debug!("Comment is ++++>>> {}", comment);
                }
                //0 => {
                //    //Obect creation expression
                //    let prefix = m.captures[0].node.utf8_text(&contents).unwrap();
                //    let name = m.captures[1].node.utf8_text(&contents).unwrap();
                //    let fqn = format!("{}\\{}", &prefix, &name);
                //    let key = if let Some(key) = m.captures.get(2) {
                //        key.node.utf8_text(&contents).unwrap()
                //    } else {
                //        name
                //    };
                //    ns_map.insert(key, fqn);

                //}
                //0 => {
                //    log::debug!("Namespace usage");
                //    let prefix = m.captures[0].node.utf8_text(&contents).unwrap();
                //    let name = m.captures[1].node.utf8_text(&contents).unwrap();
                //    let fqn = format!("{}\\{}", &prefix, &name);
                //    let key = if let Some(key) = m.captures.get(2) {
                //        key.node.utf8_text(&contents).unwrap()
                //    } else {
                //        name
                //    };
                //    ns_map.insert(key, fqn);
                //}
                //1 => {
                //    log::debug!("Detect variables assignment");

                //    let var_name = m.captures[0].node.utf8_text(&contents).unwrap();
                //    let class_name = m.captures[1].node.utf8_text(&contents).unwrap();
                //    if let Some(fqn) = ns_map.get(class_name) {
                //        //log::debug!("detected map of variable to FQN {} => {}", var_name, fqn);
                //        vars_map.insert(var_name, fqn.clone());
                //    }
                //}
                //2 => {
                //    log::debug!(
                //        "Member call expression {}, pattern_index = {}",
                //        m.captures.len(),
                //        m.pattern_index
                //    );
                //    match m.pattern_index {
                //        0 => {
                //            let var_name = m.captures[0].node.utf8_text(&contents).unwrap();
                //            let method_name = m.captures[1].node.utf8_text(&contents).unwrap();
                //            if let Some(fqn) = vars_map.get(var_name) {
                //                log::debug!(
                //                    "now serch for the location  of => {}::{}",
                //                    fqn,
                //                    method_name
                //                );
                //            }
                //        }

                //        1 | 3 => {
                //            let class_name = m.captures[0].node.utf8_text(&contents).unwrap();
                //            let method_name = m.captures[1].node.utf8_text(&contents).unwrap();
                //            if let Some(fqn) = ns_map.get(class_name) {
                //                log::debug!(
                //                    "now serch for the location  of => {}::{}",
                //                    fqn,
                //                    method_name
                //                );
                //            }
                //        }
                //        2 => {
                //            let ns_name = m.captures[0].node.utf8_text(&contents).unwrap();
                //            let class_name = m.captures[1].node.utf8_text(&contents).unwrap();
                //            let method_name = m.captures[2].node.utf8_text(&contents).unwrap();
                //            let ns_name = if let Some(fqn) = ns_map.get(ns_name) {
                //                fqn
                //            } else {
                //                ns_name
                //            };
                //            let fqn = format!("{}\\{}", ns_name, class_name);
                //            log::debug!(
                //                "now serch for the location  of => {}::{}",
                //                fqn,
                //                method_name
                //            );
                //        }
                //        _ => {}
                //    };
                //}
                _ => panic!("Somethig went wrong!!!"),
            }
        }
    }
    //for m in matches.filter(|x| x.pattern_index == 0) {
    //    log::debug!("{:#?}", m);
    //    match m.pattern_index {
    //        // namespace use mapping
    //        0 => {
    //            let prefix = m.captures[0].node.utf8_text(&contents).unwrap();
    //            let name = m.captures[1].node.utf8_text(&contents).unwrap();
    //            let fqn = format!("{}\\{}", &prefix, &name);
    //            let key = if let Some(key) = m.captures.get(2) {
    //                key.node.utf8_text(&contents).unwrap()
    //            } else {
    //                name
    //            };
    //            ns_map.insert(key, fqn);
    //        }
    //        // variable assignment mapping to FQN
    //        1 => {
    //            let var_name = m.captures[0].node.utf8_text(&contents).unwrap();
    //            let class_name = m.captures[1].node.utf8_text(&contents).unwrap();
    //            if let Some(fqn) = ns_map.get(class_name) {
    //                //log::debug!("detected map of variable to FQN {} => {}", var_name, fqn);
    //                vars_map.insert(var_name, fqn.clone());
    //            }
    //        }
    //        2 => {
    //            log::debug!("var_map {:?}", vars_map);
    //            let var_name = m.captures[0].node.utf8_text(&contents).unwrap();
    //            let method_name = m.captures[1].node.utf8_text(&contents).unwrap();
    //            if let Some(fqn) = vars_map.get(var_name) {
    //                log::debug!(
    //                    "I found a var name map, so i can search for location: {}::{}",
    //                    fqn,
    //                    method_name
    //                );
    //            }
    //        }
    //        // final step to produce FQN to search for location
    //        3 => {
    //            let class_name = m.captures[0].node.utf8_text(&contents).unwrap();
    //            let methond_name = m.captures[1].node.utf8_text(&contents).unwrap();
    //            if let Some(fqn) = ns_map.get(class_name) {
    //                log::debug!(
    //                    "now serch for the location  of => {}::{}",
    //                    fqn,
    //                    methond_name
    //                );
    //            }
    //        }

    //        _ => (),
    //    }

    //for node in m.captures {
    //    log::debug!("NODE: {:?}", node.node);
    //    let text = node.node.utf8_text(&contents[..])?;
    //    log::debug!("{}", text);
    //}
    //}
    Ok(())
}
fn print_tree<'a>(node: &tree_sitter::Node<'a>, lvl: usize) {
    let indent = " ".repeat(lvl * 2);
    println!("{}{:?}", indent, &node);
    for i in 0..node.child_count() {
        if let Some(node) = node.child(i) {
            print_tree(&node, lvl + 1);
        }
    }
}
struct MyQuery {
    queries: Vec<&'static str>,
}
