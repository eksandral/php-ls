use std::str::FromStr;
use tokio::runtime::Runtime;

use lsp_types::{Location, Position, Range, Url};
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteQueryResult, SqliteRow},
    ConnectOptions, Row, SqliteConnection,
};

use crate::indexer::index::Index;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClassRecord {
    pub id: u32,
    pub kind: ClassRecordKind,
    pub name: String,
    pub fqn: String,
    pub implements: Vec<String>,
    pub implementations: Vec<String>,
    pub location: Location,
}
#[derive(Debug, Default, Serialize, Deserialize, sqlx::Type, Clone, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ClassRecordKind {
    #[default]
    Class = 0,
    Intreface,
    Base,
    Atgtribute,
    Method,
}

impl sqlx::FromRow<'_, SqliteRow> for ClassRecord {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        let uri: &str = row.try_get::<'_, &str, &str>("location_uri")?;
        let uri: Url = Url::from_str(uri).unwrap();
        let pos_start = Position::new(
            row.try_get("location_position_start_line")?,
            row.try_get("location_position_start_character")?,
        );
        let pos_end = Position::new(
            row.try_get("location_position_end_line")?,
            row.try_get("location_position_end_character")?,
        );
        let range = Range::new(pos_start, pos_end);
        let location: Location = Location::new(uri, range);
        Ok(ClassRecord {
            id: row.try_get("id")?,
            kind: row.try_get("kind")?,
            name: row.try_get("name")?,
            fqn: row.try_get("fqn")?,
            location,
            implements: Vec::new(),
            implementations: Vec::new(),
        })
    }
}
#[derive(Debug)]
pub struct Db {
    conn: SqliteConnection,
    rt: Runtime,
}
impl Index for Db {
    type Record = ClassRecord;
    type Err = sqlx::Error;
    type SaveResult = SqliteQueryResult;
    fn save(&mut self, row: &Self::Record) -> Result<SqliteQueryResult, Self::Err> {
        self.save_row(&row)
    }
}
impl Db {
    //const CREATE_DB: &'static str = r#"CREATE DATABASE"#;
    pub fn new(filename: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let rt = Runtime::new()?;
        let conn = rt.block_on(async {
            SqliteConnectOptions::from_str(filename)?
                .journal_mode(SqliteJournalMode::Wal)
                //.read_only(true)
                .filename(filename)
                .connect()
                .await
        })?;
        Ok(Self { rt, conn })
    }
    pub async fn get_all(&mut self) -> sqlx::Result<Vec<ClassRecord>> {
        sqlx::query_as::<_, ClassRecord>("SELECT * FROM symbol")
            .fetch_all(&mut self.conn)
            .await
    }
    pub async fn get_class_by_name(&mut self, name: &str) -> sqlx::Result<Vec<ClassRecord>> {
        sqlx::query_as::<_, ClassRecord>("SELECT * FROM symbol WHERE class_name = ?")
            .bind(name)
            .fetch_all(&mut self.conn)
            .await
    }
    pub fn get_class_by_method_location(&mut self, location: &Location) -> sqlx::Result<ClassRecord> {
        self.rt.block_on(async {
            sqlx::query_as::<_, ClassRecord>(
                r#"
            SELECT * FROM symbol WHERE location_uri = ?
                AND location_position_start_line <= ?
                AND location_position_end_line  >= ?
            "#,
            )
            .bind(location.uri.to_string())
            .bind(location.range.start.line)
            .bind(location.range.end.line)
            .fetch_one(&mut self.conn)
            .await
        })
    }
    pub fn save_row(&mut self, symbol: &ClassRecord) -> sqlx::Result<SqliteQueryResult> {
        self.rt.block_on(async {
            sqlx::query(
                r#"
            INSERT INTO symbol(
                kind, 
                name, 
                fqn, 
                implements,
                implementations, 
                location_uri,
                location_position_start_line,
                location_position_start_character,
                location_position_end_line,
                location_position_end_character
            )

            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) 
            ON CONFLICT(kind, fqn, location_uri) 
            DO UPDATE SET location_position_start_line = $7,
                location_position_start_character = excluded.location_position_start_character,
                location_position_end_line = excluded.location_position_end_line,
                location_position_end_character = excluded.location_position_end_character

            "#,
            )
            .bind(symbol.kind.clone() as u8)
            .bind(symbol.name.clone())
            .bind(symbol.fqn.clone())
            .bind(symbol.implements[..].join(","))
            .bind(symbol.implementations[..].join(","))
            .bind(symbol.location.uri.to_string())
            .bind(symbol.location.range.start.line)
            .bind(symbol.location.range.start.character)
            .bind(symbol.location.range.end.line)
            .bind(symbol.location.range.end.character)
            .execute(&mut self.conn)
            .await
        })
    }
}
