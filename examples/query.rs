use core::panic;
use std::fs::read;

use tree_sitter::{Parser, Query, QueryCursor};
use tree_sitter_php::language_php;

fn main() -> anyhow::Result<()> {
    //let path = "/home/eksandral/projects/php-template/singletone.php";
    let path = "/home/eksandral/projects/php-template/src/Singletone.php";
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
    log::debug!("ROOT: {:?}", &root_node);
    let query = Query::new(language_php(), "(namespace_definition name: (_) @name) @ns (class_declaration (declaration_list (method_declaration) @method)) @cd")?;
    let mut query_cursor = QueryCursor::new();
    let matches = query_cursor.matches(&query, root_node, &contents[..]);
    //log::debug!("")
    for m in matches {
        log::debug!("{:#?}", m.captures);

        for node in m.captures {
            log::debug!("NODE: {:?}", node.node);
            let text = node.node.utf8_text(&contents[..])?;
            log::debug!("{}", text);
        }
    }
    Ok(())
}
