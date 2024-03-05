use std::{cell::RefCell, fmt::Debug, fs, path::Path};

use anyhow::{anyhow, Ok};
use lsp_types::{InitializeParams, Url};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use tree_sitter::Parser;

use crate::{db::Db, indexer::index::Indexer, ParamsGetProjectPath, DB};
thread_local! {

    pub static INDEXERS: RefCell<Vec<Box<dyn Indexer>>> = RefCell::new(vec![
       Box::new(class_declaration::ClassDeclarationIndexer::default()),
       //Box::new(function_declarion::FunctionDeclarationIndexer::default()),
        //Box::new(class_reference::ClassReferenceIndexer::default()),
    ]);
}
pub mod class_declaration;
pub mod class_reference;
pub mod function_declarion;
pub mod index;

#[derive(Debug, Default)]
pub struct EnumDeclarationIndexer {}
#[derive(Debug, Default)]
pub struct FunctionDeclarationIndexer {}
#[derive(Debug, Default)]
pub struct InterfaceDeclarationIndexer {}
#[derive(Debug, Default)]
pub struct TraitDeclarationIndexer {}
#[derive(Debug, Default)]
pub struct TraitUseClauseIndexer {}
#[derive(Debug, Default)]
pub struct ClassLikeReferenceIndexer {}
#[derive(Debug, Default)]
pub struct FunctionReferenceIndexer {}
#[derive(Debug, Default)]
pub struct ConstantDeclarationIndexer {}
#[derive(Debug, Default)]
pub struct MemberIndexer {}

pub fn reindex_project<P: AsRef<Path> + Debug>(root_path: P) -> anyhow::Result<()> {
    log::info!("Start to reindex project {:?}", &root_path);
    // get project path from init params
    {
        let mut db = Db::new(&root_path)?;
        db.setup()?;
        db.clean_index()?;
        DB.set(Some(db));
    }

    // collect all PHP files
    let files: Vec<_> = walkdir::WalkDir::new(&root_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "php"))
        .map(|file| file.path().to_str().unwrap().to_owned())
        .collect();
    // run indexers in parallel
    files.iter().for_each(move |path| {
        let mut parser = Parser::new();
        parser
            .set_language(tree_sitter_php::language_php())
            .expect("Error loading PHP parsing support");
        let contents = fs::read(&path).expect(format!("Cannot read path {}", &path).as_str());
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
                    if let Some(db) = db {
                        parse_node(&node, db, 0, &contents, &uri);
                    }
                });
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    });
    Ok(())
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
        log::debug!("RUN INDEXERS");
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
