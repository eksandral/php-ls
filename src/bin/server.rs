use std::{cell::RefCell, collections::HashMap, error::Error, fs::read_to_string, path::Path};

use crossbeam_channel::{select, Receiver};
use lsp_types::{
    request::{GotoDefinition, HoverRequest},
    CompletionItem, CompletionOptions, CompletionResponse, DeclarationCapability,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverProviderCapability, InitializeParams,
    Location, MarkupContent, OneOf, Position, ServerCapabilities, ServerInfo, SignatureHelpOptions,
    TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};

use lsp_server::{Connection, ExtractError, Message, Notification, Request, RequestId, Response};
use php_ls::debug_node;
use serde::Serialize;
use tree_sitter::{Node, Parser};
const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");
thread_local! {
    static NS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
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

    log::debug!("before connection initialize");
    //let initialization_params = connection.initialize(server_capabilities)?;

    let (id, params) = connection.initialize_start()?;

    let initialize_data = serde_json::json!({
        "capabilities": server_capabilities,
        "serverInfo": server_info
    });

    connection.initialize_finish(id, initialize_data)?;

    log::debug!("after connection initialize");
    main_loop(connection, params)?;
    io_threads.join()?;

    // Shut down gracefully.
    log::debug!("shutting down server");
    Ok(())
}

fn main_loop(
    connection: Connection,
    params: serde_json::Value,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let params: InitializeParams = serde_json::from_value(params).unwrap();
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
    //        }
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
                NS.set(HashMap::new());
                let params: GotoDefinitionParams = serde_json::from_value(params)?;
                log::debug!("PARAMS OF DEFINITION REQ: {params:#?}");
                let current_positon = params.text_document_position_params.position;
                let path = params
                    .text_document_position_params
                    .text_document
                    .uri
                    .path()
                    .to_string();
                let resutl = find_node_with_position(current_positon, path);

                log::debug!("{:#?}", resutl);
                //let list = vec![Location {
                //    uri: Url::parse("file:///home/eksandral/projects/php-template/src/Engine.php")
                //        .unwrap(),
                //    range: lsp_types::Range {
                //        start: lsp_types::Position {
                //            line: 10,
                //            character: 21,
                //        },
                //        end: Position {
                //            line: 10,
                //            character: 31,
                //        },
                //    },
                //}];
                let list = vec![];
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
    log::debug!("out = {out:?}");
    log::debug!("finish loop");

    Ok(None)
}
fn parse_node<'a>(
    node: &'a tree_sitter::Node<'a>,
    lvl: usize,
    contents: &[u8],
    positon: Position,
    ns: &mut HashMap<String, String>,
) -> Option<String> {
    //debug_node(node, lvl);
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
        "object_creation_expression"
            if (range.start_point.row == positon.line as usize
                && range.start_point.column <= positon.character as usize
                && range.end_point.row == positon.line as usize
                && range.end_point.column >= positon.character as usize) =>
        {
            log::debug!(
                "{:width$}[{lvl}]{:#?}: {:?}",
                " ",
                node.kind(),
                node.range()
            );
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "name" {
                        if let Some(obj_name) =
                            child.utf8_text(contents).ok().map(|x| x.to_string())
                        {
                            log::debug!("OBJECT CREATION EXPRESSION name = {}", &obj_name);
                            log::debug!("{:?}", &ns);
                            if let Some(fqn) = ns.get(&obj_name) {
                                return Some(fqn.to_string());
                            }
                        }
                    }
                }
            }
            return None;
        }
        _ => (),
    }
    //log::debug!("{}", node.utf8_text(contents).unwrap());
    for child in 0..node.child_count() {
        if let Some(found) =
            parse_node(&node.child(child).unwrap(), lvl + 1, &contents, positon, ns)
        {
            return Some(found);
        }
    }
    None
}
fn find_node_with_position<'a>(positon: Position, path: String) -> anyhow::Result<Option<String>> {
    parse_file(path, positon)
}
