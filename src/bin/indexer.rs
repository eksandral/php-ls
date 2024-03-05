use clap::Parser as CliParser;
use lsp_types::Url;
use php_ls::db::Db;
use php_ls::indexer::index::Indexer;
use php_ls::{indexer::*, DB};
use std::cell::RefCell;
use std::error::Error;
use std::path::PathBuf;

use tokio::runtime::Runtime;
thread_local! {

    pub static RT:  RefCell<Runtime> = RefCell::new(Runtime::new().unwrap());
    pub static INDEXERS: RefCell<Vec<Box<dyn Indexer>>> = RefCell::new(vec![
       Box::new(class_declaration::ClassDeclarationIndexer::default()),
       //Box::new(function_declarion::FunctionDeclarationIndexer::default()),
        //Box::new(class_reference::ClassReferenceIndexer::default()),
    ]);
}
fn main() -> Result<(), Box<dyn Error>> {
    let _ = dotenv::dotenv().ok();
    env_logger::init();

    let time = std::time::Instant::now();
    let args = Args::parse();
    //let mut db: DB = DB::new("php.db");

    let root_path = args.project_path.clone();
    // TAKE FUNCTION FROM src/indexer.rs

    reindex_project(&root_path)?;
    let data = DB.with_borrow_mut(|db| {
        if let Some(db) = db {
            RT.with_borrow_mut(|rt| rt.block_on(async { db.get_all().await.unwrap() }))
        } else {
            vec![]
        }
    });
    data.iter().for_each(|x| println!("{:?}", x.fqn));
    println!("Execution time {}", time.elapsed().as_secs_f32());
    //println!("indexed files {}", index.lock().unwrap().len());
    Ok(())
}

/// Simple program to greet a person
#[derive(CliParser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    project_path: PathBuf,
}

fn parse_node<'a>(
    node: &'a tree_sitter::Node<'a>,
    index: &mut Db,
    lvl: usize,
    contents: &[u8],
    uri: &Url,
) {
    //debug_node(node, lvl);
    //if node.kind_id() == 207 {
    //    println!(
    //        "----------------------------------->>>>>>>   {:?}::::: {:?},   {}",
    //        get_node_name(&node, contents),
    //        node.named_child(1).unwrap().utf8_text(contents),
    //        node.child_count(),
    //    )
    //}

    INDEXERS.with_borrow_mut(|indexers| {
        for indexer in indexers {
            indexer.index(index, &contents, node, uri);
        }
    });
    //if node.kind_id() == 1 {
    //    println!(
    //        "-------->>>>> {:?}",
    //        node.utf8_text(contents).ok().map(|x| x.to_string())
    //    );
    //}

    //let class_indexer = class_declaration::ClassDeclarationIndexer::default();
    //if class_indexer.can_index(&node) {
    //    class_indexer.index(index, &contents, node, uri);
    //}
    //let function_indxer = function_declarion::FunctionDeclarationIndexer::default();
    //if function_indxer.can_index(&node) {
    //    function_indxer.index(index, &contents, node, uri);
    //}
    //match node.kind() {
    //    "class_declaration" => {
    //        //log::debug!(
    //        //    "{:width$}[{lvl}]{:#?}: {:?}",
    //        //    " ",
    //        //    node.kind(),
    //        //    node.range()
    //        //);
    //        save_node(SymbolKind::Class, node, contents, uri);
    //    }
    //    "method_declaration" => {
    //        save_node(SymbolKind::Method, node, contents, uri);
    //    }
    //    _ => (),
    //}
    //log::debug!("{}", node.utf8_text(contents).unwrap());
    for child in 0..node.child_count() {
        parse_node(&node.child(child).unwrap(), index, lvl + 1, &contents, uri);
    }
}
