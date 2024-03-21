use dotenv::dotenv;
use lsp_types::{notification::Initialized, OneOf};
use lsp_types::{
    request::GotoDefinition, GotoDefinitionResponse, InitializeParams, ServerCapabilities,
};
use lsp_types::{Location, Position, Range, Url};
use php_ls::db::{ClassRecord, ClassRecordKind};
use std::error::Error;
use std::path::PathBuf;
use std::str::FromStr;

use lsp_server::{Connection, ExtractError, Message, Notification, Request, RequestId, Response};

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    let _ = dotenv().ok();
    env_logger::init();
    let location = Location {
        uri: Url::from_str("file:/tmp/test.php")?,
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 1,
            },
        },
    };
    let record = ClassRecord {
        id: 1309,
        fqn: "\\stdClass".to_string(),
        description: Default::default(),
        location,
        parameters: None,
        attributes: None,
        return_type: None,
    };
    let value = serde_json::to_value(record)?;
    log::debug!("{}", value);
    Ok(())
}
