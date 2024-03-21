use std::{cell::RefCell, collections::HashMap, error::Error, fmt::format, fs::read, path::Path};

use crossbeam_channel::{select, Receiver};
use log::{debug, warn};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemLabelDetails, CompletionOptions,
    CompletionParams, CompletionResponse, DeclarationCapability, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, Documentation, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverContents, HoverParams, HoverProviderCapability,
    InitializeParams, LanguageString, Location, MarkedString, MarkupContent, MarkupKind, OneOf,
    Position, Range, ServerCapabilities, ServerInfo, SignatureHelpOptions,
    TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};

use lsp_server::{Connection, ExtractError, Message, Notification, Request, RequestId, Response};
use php_ls::{
    debug_node, indexer::reindex_project, utils::PositionInRange, ParamsGetProjectPath, DB,
};
use serde::de::value;
use tree_sitter::{Parser, Query, QueryCursor, Tree};
use tree_sitter_php::language_php;

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
    let mut state = ServerState {
        params,
        current_buffer: String::new(),
        variables: HashMap::new(),
        namespaces: HashMap::new(),
    };
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
                let params: HoverParams = serde_json::from_value(params)?;
                log::debug!("Hover request received {:#?}", &params);
                let value = self.get_hover(&params.text_document_position_params.position);
                let result = serde_json::to_value(value)?;
                Ok(Some(Response::new_ok(id, result)))
            }
            Request { id, method, params } if method == "textDocument/didSave" => {
                //let params: DidSaveTextDocumentParams = serde_json::from_value(params)?;
                //self.current_buffer = String::new();

                Ok(None)
            }
            Request { id, method, params } if method == "textDocument/completion" => {
                log::debug!("PARAMS OF COMPL REQ: {params:?}");
                let params: CompletionParams = serde_json::from_value(params)?;
                let list = self.get_completions(&params.text_document_position.position);
                let value = CompletionResponse::Array(list);
                let result = serde_json::to_value(value)?;
                Ok(Some(Response::new_ok(id, result)))
            }
            Request { id, method, params } if method == "textDocument/definition" => {
                log::debug!("Received go to definition request");
                let mut list = vec![];
                NS.set(HashMap::new());
                let params: GotoDefinitionParams = serde_json::from_value(params)?;
                let current_position = params.text_document_position_params.position;
                let path = params
                    .text_document_position_params
                    .text_document
                    .uri
                    .path()
                    .to_string();
                if let Some(class_name) = detect_class_name(current_position, path) {
                    log::debug!("I found it.. Yehoo {}", &class_name);
                    if let Ok(results) = DB.with_borrow_mut(|db| {
                        if let Some(db) = db {
                            //log::debug!("BEFORE SEARCH");
                            let r = db.find_by_fqn(&class_name);
                            //log::debug!("FROM DB {:?}", r);
                            r
                        } else {
                            Ok(vec![])
                        }
                    }) {
                        results.iter().for_each(|row| {
                            list.push(row.location.clone());
                        });
                    }
                } else {
                    log::debug!("I cannot find any symbol for definition");
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
        match not {
            Notification { method, params } if method == "textDocument/didChange" => {
                let params: DidChangeTextDocumentParams = serde_json::from_value(params)?;
                let text: String = params
                    .content_changes
                    .get(0)
                    .map_or("".to_string(), |x| x.text.clone());
                self.current_buffer = text;
                self.index_current_buffer();
            }
            Notification { method, params } if method == "textDocument/didOpen" => {
                let params: DidOpenTextDocumentParams = serde_json::from_value(params)?;
                let text: String = params.text_document.text;
                self.current_buffer = text;
                self.index_current_buffer();
            }
            _ => {}
        }
        Ok(None)
    }
    fn complete_request(&mut self, resp: Response) -> anyhow::Result<Option<Response>> {
        log::debug!("git respose :{resp:?}");
        Ok(None)
    }
    fn get_hover(&mut self, position: &Position) -> Option<Hover> {
        self.index_current_buffer();
        log::debug!("vars {:?}", self.variables);
        log::debug!("namespaces {:?}", self.namespaces);
        log::debug!("buffer= {:?}", self.current_buffer);
        let tree = get_parsed_tree(self.current_buffer.as_bytes()).unwrap();
        let contents = self.current_buffer.as_bytes();
        let root_node = tree.root_node();
        let queries =
            vec![
                "(member_call_expression object:(variable_name (name) @variable_name) name: (name) @method_name) @root",
                "(object_creation_expression (name) @class_name)",
            ];
        for (idx, query) in queries.iter().enumerate() {
            //let query = query.join(" \n");
            let query = Query::new(language_php(), &query).unwrap();
            let mut query_cursor = QueryCursor::new();
            let matches = query_cursor.matches(&query, root_node, &contents[..]);
            match idx {
                0 => {
                    for m in matches {
                        if !m.captures[2].node.range().includes(position) {
                            continue;
                        }
                        // here we need to find a type of a variable
                        let var_name = m.captures[1].node.utf8_text(contents).unwrap();
                        let method_name = m.captures[2].node.utf8_text(contents).unwrap();
                        if let Some(var_type) = self.variables.get(var_name) {
                            if let Some(results) = DB.with_borrow_mut(|db| {
                                if let Some(db) = db {
                                    let results = db
                                        .find_by_fqn_like(
                                            format!("{}::{}", var_type, method_name).as_str(),
                                        )
                                        .unwrap();
                                    Some(results)
                                } else {
                                    None
                                }
                            }) {
                                //process db result
                                let hovers: Vec<Hover> = results
                                    .iter()
                                    .map(|x| {
                                        let method_signature = format!(
                                            "<?php function {}{}{} ?>\n\n---",
                                            method_name,
                                            x.parameters.as_ref().unwrap_or(&"()".to_string()),
                                            x.return_type
                                                .as_ref()
                                                .map_or("".to_string(), |rt| format!(": {}", rt))
                                        );
                                        let contents = HoverContents::Array(vec![
                                            MarkedString::LanguageString(LanguageString {
                                                value: method_signature,
                                                language: "php".to_string(),
                                            }),
                                            MarkedString::String(x.description.clone()),
                                        ]);

                                        Hover {
                                            contents,
                                            range: None,
                                        }
                                    })
                                    .collect();
                                return hovers.first().map(|x| x.clone());
                            }
                        };
                    }
                }
                1 => {
                    for m in matches {
                        if !m.captures[0].node.range().includes(position) {
                            continue;
                        }

                        let class_name = m.captures[0].node.utf8_text(contents).unwrap();
                        return self
                            .namespaces
                            .get(class_name)
                            .map(|class_fqn| {
                                DB.with_borrow_mut(|db| {
                                    db.as_mut()
                                        .map(|db| db.find_one_by_fqn(class_fqn).ok())
                                        .flatten()
                                        .map(|x| {
                                            let (ns_name,class_name) = &class_fqn.rsplit_once("\\").unwrap_or(("",""));
                                            let class_signature = format!(
                                                "<?php\nnamespace {}\nclass {}\n---",
                                                ns_name, class_name
                                            );
                                            let contents = HoverContents::Array(vec![
                                                MarkedString::LanguageString(LanguageString {
                                                    value: class_signature,
                                                    language: "php".to_string(),
                                                }),
                                                MarkedString::String(x.description.clone()),
                                            ]);

                                            Hover {
                                                contents,
                                                range: None,
                                            }
                                        })
                                })
                            })
                            .flatten();
                    }
                }
                _ => (),
            }
        }
        // if nothing found we return no hover
        None
    }
    fn get_completions(&mut self, position: &Position) -> Vec<CompletionItem> {
        self.index_current_buffer();
        log::debug!("vars {:?}", self.variables);
        log::debug!("namespaces {:?}", self.namespaces);
        log::debug!("buffer= {:?}", self.current_buffer);
        let tree = get_parsed_tree(self.current_buffer.as_bytes()).unwrap();
        let contents = self.current_buffer.as_bytes();
        let root_node = tree.root_node();
        let queries =
            vec!["(member_access_expression object:(variable_name (name) @variable_name)) @root"];
        for (idx, query) in queries.iter().enumerate() {
            //let query = query.join(" \n");
            let query = Query::new(language_php(), &query).unwrap();
            let mut query_cursor = QueryCursor::new();
            let matches = query_cursor.matches(&query, root_node, &contents[..]);

            for m in matches {
                log::debug!("MATCH {:?}", m);
                if !m.captures[0].node.range().includes(position) {
                    continue;
                }
                // here we need to find a type of a variable
                let var_name = m.captures[1].node.utf8_text(contents).unwrap();
                log::debug!("var name is  {}", &var_name);
                if let Some(var_type) = self.variables.get(var_name) {
                    log::debug!("I found type name {}", var_type);
                    if let Some(results) = DB.with_borrow_mut(|db| {
                        if let Some(db) = db {
                            let results = db
                                .find_by_fqn_like(format!("{}::%", var_type).as_str())
                                .unwrap();
                            Some(results)
                        } else {
                            None
                        }
                    }) {
                        //process db result
                        return results
                            .iter()
                            .map(|x| {
                                //use label detail to show return type
                                let method_name = &x.fqn[var_type.len() + 2..];
                                let label_details = Some(CompletionItemLabelDetails {
                                    detail: x
                                        .return_type
                                        .clone()
                                        .zip(x.parameters.clone())
                                        .zip(Some(method_name))
                                        .map(|((params, ret), method)| {
                                            format!("{}{}: {}", method, ret, params)
                                        }), // function signature
                                    description: None, // filepath
                                });
                                let detail = None; //Some(x.fqn.clone());
                                let documentation =
                                    Some(Documentation::MarkupContent(MarkupContent {
                                        kind: MarkupKind::Markdown,
                                        value: x.description.clone(),
                                    }));
                                let insert_text = Some(format!("{}()", method_name));
                                CompletionItem {
                                    label: format!(
                                        "{}",
                                        &method_name, // +2 means that we need to
                                                      // remove :: in the FQN too
                                                      //&x.parameters.clone().unwrap_or("()".into())
                                    ),
                                    label_details,
                                    kind: Some(CompletionItemKind::METHOD),
                                    detail,
                                    documentation,
                                    insert_text,
                                    ..Default::default()
                                }
                            })
                            .collect();
                    }
                };
            }
        }
        vec![]
    }
    fn index_current_buffer(&mut self) {
        let contents = self.current_buffer.as_bytes();
        let tree = get_parsed_tree(self.current_buffer.as_bytes()).unwrap();
        let root_node = tree.root_node();
        let queries = vec![
        // Namespace detection
        vec!["(namespace_use_declaration (namespace_use_clause (qualified_name(namespace_name_as_prefix (namespace_name) @ns_name) (name) @class_name) (namespace_aliasing_clause (name) @alias)?))",],
        vec![
            "(assignment_expression left:( (variable_name (name) @var_name) right:(object_creation_expression (name) @object_name)))",
               "(assignment_expression 
                left: (variable_name (name) @var_name)  
                right: (object_creation_expression 
                            (qualified_name (namespace_name_as_prefix (namespace_name) @ns_name) (name) @object_name)
                ))",
        ],
    ];
        let mut ns_map = HashMap::new();
        let mut vars_map = HashMap::new();
        for (idx, query) in queries.iter().enumerate() {
            let query = query.join(" \n");
            let query = Query::new(language_php(), &query).ok().unwrap();
            let mut query_cursor = QueryCursor::new();
            let matches = query_cursor.matches(&query, root_node, &contents[..]);

            for m in matches {
                match idx {
                    0 => {
                        let prefix = m.captures[0].node.utf8_text(&contents).unwrap();
                        let name = m.captures[1].node.utf8_text(&contents).unwrap();
                        let fqn = format!("{}\\{}", &prefix, &name);
                        let key = if let Some(key) = m.captures.get(2) {
                            key.node.utf8_text(&contents).unwrap()
                        } else {
                            name
                        };
                        ns_map.insert(key.to_string(), fqn);
                    }
                    1 => {
                        log::debug!("Defining variables {}", m.captures.len());
                        let var_name = m.captures[0].node.utf8_text(&contents).unwrap();
                        if m.captures.len() > 2 {
                            let ns_name = m.captures[1].node.utf8_text(&contents).unwrap();
                            let class_name = m.captures[2].node.utf8_text(&contents).unwrap();
                            log::debug!(
                                "detected obect creation with namespace: {} ; {}",
                                ns_name,
                                class_name
                            );
                            let fqn = if let Some(fqn) = ns_map.get(ns_name) {
                                log::debug!(
                                    "detected map of variable to FQN {} => {}",
                                    var_name,
                                    fqn
                                );
                                format!("{}\\{}", fqn, class_name)
                            } else {
                                format!("{}\\{}", ns_name, class_name)
                            };
                            vars_map.insert(var_name.to_string(), fqn.clone());
                        } else {
                            let class_name_node = m.captures[1].node;

                            let class_name = m.captures[1].node.utf8_text(&contents).unwrap();
                            if let Some(fqn) = ns_map.get(class_name) {
                                log::debug!(
                                    "detected map of variable to FQN {} => {}",
                                    var_name,
                                    fqn
                                );
                                vars_map.insert(var_name.to_string(), fqn.clone());
                            }
                        }
                    }
                    _ => panic!("Somethig went wrong!!!"),
                }
            }
        }
        self.variables = vars_map;
        self.namespaces = ns_map;
    }
}
struct ServerState {
    params: InitializeParams,
    current_buffer: String,
    namespaces: HashMap<String, String>,
    variables: HashMap<String, String>,
}

fn get_parsed_tree(source: &[u8]) -> Option<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(tree_sitter_php::language_php())
        .expect("Error loading PHP parsing support");
    parser.parse(&source, None)
}
fn detect_class_name(position: Position, path: String) -> Option<String> {
    log::debug!("parsing a path {:?}", path);
    let contents = read(&path).ok()?;
    let tree = get_parsed_tree(&contents[..])?;
    search_member_call_expressions(&tree, &contents, &position)
}

fn search_member_call_expressions(
    tree: &tree_sitter::Tree,
    contents: &[u8],
    position: &Position,
) -> Option<String> {
    let root_node = tree.root_node();
    let queries = vec![
        // Namespace detection
        vec!["(namespace_use_declaration (namespace_use_clause (qualified_name(namespace_name_as_prefix (namespace_name) @ns_name) (name) @class_name) (namespace_aliasing_clause (name) @alias)?))",],
        vec![
            "(assignment_expression left:( (variable_name (name) @var_name) right:(object_creation_expression (name) @object_name)))",
               "(assignment_expression 
                left: (variable_name (name) @var_name)  
                right: (object_creation_expression 
                            (qualified_name (namespace_name_as_prefix (namespace_name) @ns_name) (name) @object_name)
                ))",
        ],
        vec!["(member_call_expression (variable_name (name) @varname) (name) @method_name)",
        "(member_call_expression (parenthesized_expression (object_creation_expression (name) @class_name) ) (name) @method_name)",
      "
(member_call_expression 
    (parenthesized_expression 
        (object_creation_expression 
            (qualified_name 
                (namespace_name_as_prefix)? @ns_as_prefix 
                (name) @cass_name
            ) 
        ) 
    ) 
    (name) @method_name
)",
            "(scoped_call_expression (name) @class_name (name) @method_name)"
        ],
        // Object creation expression
        vec!["(object_creation_expression (qualified_name (namespace_name_as_prefix (namespace_name) @ns_name) (name) @class_name))"]
    ];
    let mut ns_map = HashMap::new();
    let mut vars_map = HashMap::new();
    for (idx, query) in queries.iter().enumerate() {
        let query = query.join(" \n");
        let query = Query::new(language_php(), &query).ok()?;
        let mut query_cursor = QueryCursor::new();
        let matches = query_cursor.matches(&query, root_node, &contents[..]);

        for m in matches {
            match idx {
                0 => {
                    let prefix = m.captures[0].node.utf8_text(&contents).unwrap();
                    let name = m.captures[1].node.utf8_text(&contents).unwrap();
                    let fqn = format!("{}\\{}", &prefix, &name);
                    let key = if let Some(key) = m.captures.get(2) {
                        key.node.utf8_text(&contents).unwrap()
                    } else {
                        name
                    };
                    ns_map.insert(key, fqn);
                }
                1 => {
                    log::debug!("Defining variables {}", m.captures.len());
                    let var_name = m.captures[0].node.utf8_text(&contents).unwrap();
                    if m.captures.len() > 2 {
                        let ns_name = m.captures[1].node.utf8_text(&contents).unwrap();
                        let class_name = m.captures[2].node.utf8_text(&contents).unwrap();
                        log::debug!(
                            "detected obect creation with namespace: {} ; {}",
                            ns_name,
                            class_name
                        );
                        let fqn = if let Some(fqn) = ns_map.get(ns_name) {
                            log::debug!("detected map of variable to FQN {} => {}", var_name, fqn);
                            format!("{}\\{}", fqn, class_name)
                        } else {
                            format!("{}\\{}", ns_name, class_name)
                        };
                        if m.captures[2].node.range().includes(position) {
                            return Some(fqn.to_string());
                        }
                        vars_map.insert(var_name, fqn.clone());
                    } else {
                        let class_name_node = m.captures[1].node;

                        log::debug!(
                            "object creation without namespace {:?}",
                            class_name_node.range().includes(position)
                        );
                        let class_name = m.captures[1].node.utf8_text(&contents).unwrap();
                        if let Some(fqn) = ns_map.get(class_name) {
                            if class_name_node.range().includes(position) {
                                return Some(fqn.to_string());
                            }
                            log::debug!("detected map of variable to FQN {} => {}", var_name, fqn);
                            vars_map.insert(var_name, fqn.clone());
                        }
                    }
                }
                2 => {
                    log::debug!("vars_map = {:?}", &vars_map);
                    log::debug!("ns_map = {:?}", &ns_map);
                    log::debug!("position = {:?}", &position);
                    log::debug!("node position = {:?}", m.captures[1].node.range());
                    let found = match m.pattern_index {
                        0 if m.captures[1].node.range().includes(&position) => {
                            let var_name = m.captures[0].node.utf8_text(&contents).unwrap();
                            let method_name = m.captures[1].node.utf8_text(&contents).unwrap();
                            log::debug!("Expecting {}->{}()", &var_name, &method_name);
                            vars_map
                                .get(var_name)
                                .map(|fqn| format!("{}::{}", fqn, method_name))
                        }

                        1 | 3
                            if m.captures[0].node.range().includes(position)
                                || m.captures[1].node.range().includes(position) =>
                        {
                            let class_name_node = m.captures[0].node;
                            let method_name_node = m.captures[1].node;
                            let class_name = class_name_node.utf8_text(&contents).unwrap();
                            let method_name = method_name_node.utf8_text(&contents).unwrap();
                            log::debug!("Expecting (new {}())->{}()", &class_name, &method_name);
                            ns_map.get(class_name).map(|fqn| {
                                if class_name_node.range().includes(position) {
                                    fqn.to_string()
                                } else {
                                    format!("{}::{}", fqn, method_name)
                                }
                            })
                        }

                        2 if m.captures[2].node.range().includes(&position) => {
                            let ns_name = m.captures[0].node.utf8_text(&contents).unwrap();
                            let class_name = m.captures[1].node.utf8_text(&contents).unwrap();
                            let method_name = m.captures[2].node.utf8_text(&contents).unwrap();
                            log::debug!(
                                "Expecting (new {}\\{}())->{}()",
                                &ns_name,
                                &class_name,
                                &method_name
                            );
                            let ns_name = if let Some(fqn) = ns_map.get(ns_name) {
                                fqn
                            } else {
                                ns_name
                            };
                            Some(format!("{}{}::{}", ns_name, class_name, method_name))
                        }
                        _ => None,
                    };
                    // if we found a class name then simply return
                    // if not continue searching, because we can find in next match
                    log::debug!("FOUND {:?}", found);
                    if found.is_some() {
                        return found;
                    }
                }
                3 => {
                    log::debug!("searching for object creation references");
                    let ns_name = m.captures[0].node.utf8_text(&contents).unwrap();
                    let class_name = m.captures[1].node.utf8_text(&contents).unwrap();
                    let ns_name = ns_map
                        .get(ns_name)
                        .map_or(ns_name.to_string(), |x| x.to_string());
                    return Some(format!("{}\\{}", ns_name, class_name));
                }
                _ => panic!("Somethig went wrong!!!"),
            }
        }
    }
    None
}
