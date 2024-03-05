use std::{
    cell::RefCell,
    collections::HashMap,
    error::Error,
    fs::{read, read_to_string},
    path::Path,
};

use crossbeam_channel::{select, Receiver};
use lsp_types::{
    CompletionItem, CompletionOptions, CompletionResponse, DeclarationCapability,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverProviderCapability, InitializeParams,
    Location, MarkupContent, OneOf, Position, Range, ServerCapabilities, ServerInfo,
    SignatureHelpOptions, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};

use lsp_server::{Connection, ExtractError, Message, Notification, Request, RequestId, Response};
use php_ls::{debug_node, indexer::reindex_project, utils::PositionInRange, ParamsGetProjectPath, DB};
use tree_sitter::Parser;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");
thread_local! {
    static NS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    static VARS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    //let _ = dotenv::dotenv().ok();
    log4rs::init_file("/home/eksandral/log4rs.yml", Default::default())?;
    //env_logger::init();
    // Note that  we must have our logging only write out to stderr.
    log::debug!("starting generic LSP server");
    // Create the transport. Includes the stdio (stdin and stdout) versions but this could
    // also be implemented to use sockets or HTTP.
    let (connection, io_threads) = Connection::stdio();
    log::debug!("connection is created");
    // Run the server and wait for the two threads to end (typically by trigger LSP Exit event).
    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        definition_provider: Some(OneOf::Left(true)),
        declaration_provider: Some(DeclarationCapability::Simple(true)),
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(true),
            trigger_characters: None,
            ..Default::default()
        }),
        signature_help_provider: Some(SignatureHelpOptions::default()),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        ..Default::default()
    })?;
    let server_info = serde_json::to_value(&ServerInfo {
        name: NAME.to_string(),
        version: Some(VERSION.to_string()),
    })?;

    let (id, params) = connection.initialize_start()?;

    let initialize_data = serde_json::json!({
        "capabilities": server_capabilities,
        "serverInfo": server_info
    });

    connection.initialize_finish(id, initialize_data)?;
    let params: InitializeParams = serde_json::from_value(params)?;

    let root_path = params.get_project_path()?;
    // Reindex project
    reindex_project(&root_path)?;
    // Run main loop
    main_loop(connection, params)?;
    io_threads.join()?;

    // Shut down gracefully.
    log::debug!("shutting down server");
    Ok(())
}

fn main_loop(
    connection: Connection,
    params: InitializeParams,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let mut state = ServerState { params };
    log::debug!("starting example main loop");
    while let Some(msg) = next_event(&connection.receiver) {
        let result = match msg {
            Message::Request(req) => state.on_new_request(req),
            Message::Notification(not) => state.on_notification(not),
            Message::Response(resp) => state.complete_request(resp),
        };
        if let Ok(some) = result {
            if let Some(response) = some {
                connection.sender.send(Message::Response(response))?;
            }
        }
    }
    //for msg in &connection.receiver {
    //    log::debug!("got msg: {:?}", msg);
    //    match msg {
    //        Message::Request(req) => {
    //            if connection.handle_shutdown(&req)? {
    //                log::info!("Received shutdown requets: Bye.");
    //                return Ok(());
    //            }
    //            log::debug!("got request: {:?}", req);
    //            match cast::<GotoDefinition>(req) {
    //                Ok((id, params)) => {
    //                    log::debug!("got gotoDefinition request #{}: {:?}", id, params);
    //                    let result = Some(GotoDefinitionResponse::Array(Vec::new()));
    //                    let result = serde_json::to_value(&result).unwrap();
    //                    let resp = Response {
    //                        id,
    //                        result: Some(result),
    //                        error: None,
    //                    };
    //                    connection.sender.send(Message::Response(resp))?;
    //                    continue;
    //                }
    //                Err(err @ ExtractError::JsonError { .. }) => panic!("{:?}", err),
    //                Err(ExtractError::MethodMismatch(req)) => req,
    //            };
    //            // ...
    //        }
    //        Message::Response(resp) => {
    //            log::debug!("got response: {:?}", resp);
    //        }: 'name' was just an example
    //        Message::Notification(not) => {
    //            log::debug!("got notification: {:?}", not);
    //        }
    //    }
    //}
    Ok(())
}

fn cast<R>(req: Request) -> Result<(RequestId, R::Params), ExtractError<Request>>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}
fn next_event(inbox: &Receiver<lsp_server::Message>) -> Option<Message> {
    select! {
    recv(inbox) -> msg =>
             msg.ok()
    }
}
impl ServerState {
    fn on_new_request(&mut self, req: Request) -> anyhow::Result<Option<Response>> {
        match req {
            Request { id, method, params } if method == "textDocument/hover" => {
                let contents = lsp_types::HoverContents::Markup(MarkupContent {
                    kind: lsp_types::MarkupKind::PlainText,
                    value: "Test string  from hover".to_string(),
                });
                let range: Option<lsp_types::Range> = None;
                let value = Some(Hover { contents, range });
                let result = serde_json::to_value(value)?;
                Ok(Some(Response::new_ok(id, result)))
            }
            Request { id, method, params } if method == "textDocument/completion" => {
                log::debug!("PARAMS OF COMPL REQ: {params:?}");
                let list = vec!["array", "array_chunk", "array_column"]
                    .iter()
                    .map(|x| CompletionItem {
                        label: x.to_string(),
                        ..Default::default()
                    })
                    .collect();
                let value = CompletionResponse::Array(list);
                let result = serde_json::to_value(value)?;
                Ok(Some(Response::new_ok(id, result)))
            }
            Request { id, method, params } if method == "textDocument/definition" => {
                let mut list = vec![];
                NS.set(HashMap::new());
                let params: GotoDefinitionParams = serde_json::from_value(params)?;
                let current_positon = params.text_document_position_params.position;
                let path = params
                    .text_document_position_params
                    .text_document
                    .uri
                    .path()
                    .to_string();
                if let Some(class_name) = detect_class_name(current_positon, path)? {
                    if let Ok(results) = DB.with_borrow_mut(|db| {
                        if let Some(db) = db {
                            log::debug!("BEFORE SEARCH");
                            let r = db.find_by_fqn(&class_name);
                            log::debug!("FROM DB {:?}", r);
                            r
                        } else {
                            Ok(vec![])
                        }
                    }) {
                        results.iter().for_each(|row| {
                            list.push(row.location.clone());
                        });
                    }
                    //if let Some(location) = self.find_location(class_name) {
                    //    list.push(location);
                    //}
                }
                let value = GotoDefinitionResponse::Array(list);
                let result = serde_json::to_value(value)?;
                Ok(Some(Response::new_ok(id, result)))
            }
            _ => Ok(Some(Response::new_err(
                req.id,
                lsp_server::ErrorCode::InvalidRequest as i32,
                "Unsupported Request".to_string(),
            ))),
        }
    }
    fn on_notification(&mut self, not: Notification) -> anyhow::Result<Option<Response>> {
        log::debug!("got notification {not:?}");
        Ok(None)
    }
    fn complete_request(&mut self, resp: Response) -> anyhow::Result<Option<Response>> {
        log::debug!("git respose :{resp:?}");
        Ok(None)
    }

    fn find_location(&mut self, class_name: String) -> Option<Location> {
        let root_path = self
            .params
            .root_uri
            .clone()
            .unwrap()
            .to_file_path()
            .unwrap();
        let files: Vec<_> = walkdir::WalkDir::new(&root_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "php"))
            .map(|file| file.path().to_str().unwrap().to_owned())
            .collect();
        //let mut index_clone = index.clone();
        for path in files.iter() {
            let mut parser = Parser::new();
            parser
                .set_language(tree_sitter_php::language_php())
                .expect("Error loading PHP parsing support");
            let contents = match read(&path) {
                Ok(out) => out,
                Err(e) => {
                    println!("ERROR:{}", e);
                    println!("{}", path.clone());
                    continue;
                }
            };
            let parsed = parser.parse(&contents, None);
            let tree = if let Some(tree) = parsed {
                tree
            } else {
                log::error!("I cannot parse {:?}", &path);
                continue;
            };
            let root_node = tree.root_node();
            let mut ns = None;
            for i in 0..root_node.child_count() {
                if let Some(child) = root_node.child(i) {
                    //log::debug!("ROOT NODE CHILD: {:?} {:?}", child, child.kind());
                    if child.kind() == "namespace_definition" {
                        for n in 0..child.child_count() {
                            if let Some(child) = child.child(n) {
                                if child.kind() == "namespace_name" {
                                    log::debug!("{:?} === >>> {:?}", child, child.kind());
                                    log::debug!(
                                        "------>>> NS DEF {:?}",
                                        child.utf8_text(&contents)
                                    );
                                    ns = child.utf8_text(&contents).ok();
                                }
                            }
                        }
                    }
                    if child.kind() == "class_declaration" {
                        for n in 0..child.child_count() {
                            if let Some(child) = child.child(n) {
                                if child.kind() == "name" {
                                    log::debug!("{:?} === >>> {:?}", child, child.kind());
                                    if let Some(ns) = ns {
                                        if let Some(name) = child.utf8_text(&contents).ok() {
                                            let fqn = format!("{ns}\\{name}");
                                            log::debug!(
                                                "FQN of found class = {}, {:?}",
                                                &fqn,
                                                fqn == class_name
                                            );
                                            log::debug!(
                                                "URL {:?}: {:?}",
                                                &path,
                                                Url::from_file_path(path)
                                            );
                                            if fqn == class_name {
                                                let location = Location {
                                                    uri: Url::from_file_path(path).unwrap(),

                                                    range: Range {
                                                        start: Position {
                                                            line: child.start_position().row as u32,
                                                            character: child.start_position().column
                                                                as u32,
                                                        },
                                                        end: Position {
                                                            line: child.end_position().row as u32,
                                                            character: child.end_position().column
                                                                as u32,
                                                        },
                                                    },
                                                };
                                                log::debug!("prepared location {:?}", &location);
                                                return Some(location);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            //{
            //    let mut cursor = tree.walk();
            //    cursor.goto_first_child();
            //    loop {
            //        let node = cursor.node();
            //        if !cursor.goto_next_sibling() {
            //            break;
            //        }
            //    }
            //}
        }
        None
    }
}
struct ServerState {
    params: InitializeParams,
}

fn parse_file<'a, P>(path: P, positon: Position) -> anyhow::Result<Option<String>>
where
    P: AsRef<Path> + std::fmt::Debug,
{
    log::debug!("parsting path {:?}", path);
    let mut parser = Parser::new();
    parser
        .set_language(tree_sitter_php::language_php())
        .expect("Error loading PHP parsing support");
    let contents = read_to_string(&path)?;
    let parsed = parser.parse(&contents, None);
    let tree = if let Some(tree) = parsed {
        tree
    } else {
        log::debug!("I cannot parse {:?}", &path);
        return Err(anyhow::anyhow!("Pasre is faild"));
    };
    let mut cursor = tree.walk();
    cursor.goto_first_child();
    let mut out = None;
    let mut ns = HashMap::new();
    // 1. Detect FQN of the class
    loop {
        let node = cursor.node();

        if let Some(node) = parse_node(&node, 0, contents.as_bytes(), positon, &mut ns) {
            out = Some(node);
            break;
        }
        if !cursor.goto_next_sibling() {
            break;
        }
    }
    Ok(out)
}
fn parse_node<'a>(
    node: &'a tree_sitter::Node<'a>,
    lvl: usize,
    contents: &[u8],
    position: Position,
    ns: &mut HashMap<String, String>,
) -> Option<String> {
    debug_node(node, lvl);
    let width = lvl * 4;
    let range = node.range();
    match node.kind() {
        "namespace_use_clause" => {
            let mut ns_fqn = None;
            let mut ns_alias = None;
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "qualified_name" {
                        ns_fqn = child.utf8_text(contents).ok().map(|x| x.to_string());
                        if let Some(child) = child.child(1) {
                            ns_alias = child.utf8_text(contents).ok().map(|x| x.to_string());
                        }
                    }
                    if child.kind() == "namespace_aliasing_clause" {
                        if let Some(child) = child.child(1) {
                            ns_alias = child.utf8_text(contents).ok().map(|x| x.to_string());
                        }
                    }
                }
            }
            log::debug!("MAP OF NS ALIAS TO FQN {ns_alias:?} ==>>>> {ns_fqn:?}");
            if let Some(key) = ns_alias {
                ns.entry(key).or_insert(ns_fqn.unwrap());
            }
        }
        "object_creation_expression" => {
            //log::debug!(
            //    "{:width$}[{lvl}]{:#?}: {:?}",
            //    " ",
            //    node.kind(),
            //    node.range()
            //);
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "name" {
                        if let Some(obj_name) =
                            child.utf8_text(contents).ok().map(|x| x.to_string())
                        {
                            log::debug!("OBJECT CREATION EXPRESSION name = {}", &obj_name);
                            log::debug!("{:?}", &ns);
                            if let Some(fqn) = ns.get(&obj_name) {
                                // before return FQN lets map a variable to this fqn
                                // search for assignment_expression
                                if let Some(parent) = node.parent() {
                                    if parent.kind() == "assignment_expression" {
                                        if let Some(var_node) = parent.child(0) {
                                            if let Some(name) = var_node.child(1) {
                                                let var_name = name
                                                    .utf8_text(contents)
                                                    .ok()
                                                    .unwrap_or_default();
                                                VARS.with_borrow_mut(|vars| {
                                                    vars.entry(var_name.to_string())
                                                        .or_insert(fqn.to_string());
                                                })
                                            }
                                        }
                                    }
                                }
                                if range.includes(&position) {
                                    return Some(fqn.to_string());
                                }
                            }
                        }
                    }
                }
            }
            return None;
        }
        "scoped_call_expression" => {}
        "member_call_expression" if range.includes(&position) => {
            log::debug!("this is the member");
            if let Some(name) = node.child(0) {
                let type_name = match name.kind() {
                    "variable_name" => {
                        log::debug!("i found a var name of the member call");
                        let var_name = name.utf8_text(contents).ok().unwrap_or_default();
                        log::debug!("this is a variable {}", &var_name[1..]);
                        VARS.with_borrow(|x| {
                            log::debug!("{:?}", x);
                            x.get(&var_name[1..]).map(|x| x.to_string())
                        })
                    }
                    "scoped_call_expression" => {
                        if let Some(child) = name.child(0) {
                            let obj_name = child.utf8_text(contents).ok().unwrap_or_default();
                            log::debug!("NAMESAPCES {:?}; Obj name {:?}", ns, obj_name);
                            ns.get(obj_name).map(|x| x.to_string())
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                log::debug!("Detected type name is  {:?}", type_name);
                if let Some(type_name) = type_name {
                    if let Some(name) = node.child(2).filter(|x| x.range().includes(&position)) {
                        if let Some(method_name) = name.utf8_text(contents).ok() {
                            log::debug!("{}::{}", type_name, method_name);
                            return Some(format!("{}::{}", type_name, method_name));
                        }
                    }
                }
            }
        }
        _ => (),
    }
    //log::debug!("{}", node.utf8_text(contents).unwrap());
    for child in 0..node.child_count() {
        if let Some(found) = parse_node(
            &node.child(child).unwrap(),
            lvl + 1,
            &contents,
            position,
            ns,
        ) {
            return Some(found);
        }
    }
    None
}
fn detect_class_name(positon: Position, path: String) -> anyhow::Result<Option<String>> {
    parse_file(path, positon)
}
