use clap::Parser as CliParser;
use lsp_types::{GotoDefinitionResponse, Location, Position, Range, Url};
use php_ls::db::Db;
use php_ls::indexer::index::{Index, Indexer};
use php_ls::{debug_node, indexer::*};
use php_ls::{get_node_name, Symbol, SymbolKind};
use rayon::prelude::*;
use std::cell::RefCell;
use std::error::Error;
use std::fs::read_to_string;
use std::path::PathBuf;

use tokio::runtime::Runtime;
use tree_sitter::Parser;
thread_local! {

    pub static RT:  RefCell<Runtime> = RefCell::new(Runtime::new().unwrap());
    pub static DB: RefCell<php_ls::db::Db> = RefCell::new(php_ls::db::Db::new("php.db").unwrap());
    pub static INDEXERS: RefCell<Vec<Box<dyn Indexer>>> = RefCell::new(vec![
       //Box::new(class_declaration::ClassDeclarationIndexer::default()),
       //Box::new(function_declarion::FunctionDeclarationIndexer::default()),
        Box::new(class_reference::ClassReferenceIndexer::default()),
    ]);
}
fn main() -> Result<(), Box<dyn Error>> {
    let _ = dotenv::dotenv().ok();
    env_logger::init();

    let time = std::time::Instant::now();
    let args = Args::parse();
    //let mut db: DB = DB::new("php.db");

    let root_path = args.project_path.clone();
    let files: Vec<_> = walkdir::WalkDir::new(&root_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "php"))
        .map(|file| file.path().to_str().unwrap().to_owned())
        .collect();
    //let mut index_clone = index.clone();
    files.into_par_iter().for_each(move |path| {
        let mut parser = Parser::new();
        parser
            .set_language(tree_sitter_php::language_php())
            .expect("Error loading PHP parsing support");
        let contents = match read_to_string(&path) {
            Ok(out) => out,
            Err(e) => {
                println!("ERROR:{}", e);
                println!("{}", path.clone());
                return;
            }
        };
        let parsed = parser.parse(&contents, None);
        let tree = if let Some(tree) = parsed {
            tree
        } else {
            log::error!("I cannot parse {:?}", &path);
            return;
        };
        let uri = Url::from_file_path(path).unwrap();
        {
            let mut cursor = tree.walk();
            cursor.goto_first_child();
            loop {
                let node = cursor.node();
                DB.with_borrow_mut(|db| {
                    parse_node(&node, db, 0, contents.as_bytes(), &uri);
                });
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        //let _text_document = TextDocumentItem::new(uri.clone(), "php".to_string(), 0, contents);
        //let identifier = TextDocumentIdentifier::new(uri.clone());
        //index_clone
        //    .lock()
        //    .unwrap()
        //    //.entry(text_document.language_id.clone())
        //    .entry(uri.to_string())
        //    .or_default()
        //    .push(tree);
        //        println!("Indexed file: {}", text_document.uri);
    });
    let data = DB.with_borrow_mut(|db| {
        RT.with_borrow_mut(|rt| rt.block_on(async { db.get_all().await.unwrap() }))
    });
    data.iter().for_each(|x| println!("{:?}", x.name));
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
    debug_node(node, lvl);
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

