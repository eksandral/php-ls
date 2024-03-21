use anyhow::anyhow;
use std::{
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    str::FromStr,
    sync::Arc,
};
use tokio::runtime::Runtime;

use lsp_types::{Location, Position, Range, Url};
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteQueryResult, SqliteRow},
    ConnectOptions, Executor, Row, SqliteConnection,
};

use crate::indexer::index::Index;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClassRecord {
    pub id: u32,
    pub fqn: String,
    pub description: String,
    pub attributes: Option<String>,
    pub parameters: Option<String>,
    pub return_type: Option<String>,
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
            fqn: row.try_get("fqn")?,
            description: row.try_get("description")?,
            location,
            parameters: row.try_get("parameters")?,
            attributes: row.try_get("attributes")?,
            return_type: row.try_get("return_type")?,
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
    const FILENAME: &'static str = "php-ls.db";
    //const CREATE_DB: &'static str = r#"CREATE DATABASE"#;
    pub fn new<P: AsRef<Path>>(dirpath: P) -> Result<Self, anyhow::Error> {
        let filename = Db::get_db_filename(dirpath.as_ref())?;
        let filename = filename
            .to_str()
            .ok_or(anyhow!("I cannot convert PathBuf to str"))?;
        log::debug!("DATABASE FILE PATH  {}", &filename);
        let rt = Runtime::new()?;
        let conn = rt.block_on(async {
            SqliteConnectOptions::from_str(filename)?
                .journal_mode(SqliteJournalMode::Wal)
                .create_if_missing(true)
                .filename(filename)
                .connect()
                .await
        })?;
        Ok(Self { rt, conn })
    }

    fn get_db_filename<P: AsRef<Path>>(dirpath: P) -> anyhow::Result<PathBuf> {
        fs::create_dir_all(dirpath.as_ref())?;
        let mut filepath = dirpath.as_ref().to_path_buf();
        filepath.push(Db::FILENAME);
        Ok(filepath)
    }
    pub fn setup(&mut self) -> sqlx::Result<SqliteQueryResult> {
        let query = r#"
CREATE TABLE IF NOT EXISTS fqn_declaration(
    id INTEGER NOT NULL PRIMARY KEY,
    fqn TEXT NOT NULL ,
    description TEXT NOT NULL,
    attributes TEXT,
    parameters TEXT,
    return_type TEXT,
    location_uri TEXT,
    location_position_start_line INTEGER,
    location_position_start_character INTEGER,
    location_position_end_line INTEGER,
    location_position_end_character INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS unique_fqn_declaration
ON fqn_declaration(fqn,location_uri);
        "#;

        self.rt.block_on(async { self.conn.execute(query).await })
    }
    pub fn clean_index(&mut self) -> sqlx::Result<SqliteQueryResult> {
        let query = r#"DELETE FROM fqn_declaration;"#;

        self.rt.block_on(async { self.conn.execute(query).await })
    }

    pub async fn get_all(&mut self) -> sqlx::Result<Vec<ClassRecord>> {
        sqlx::query_as::<_, ClassRecord>("SELECT * FROM fqn_declaration")
            .fetch_all(&mut self.conn)
            .await
    }
    pub fn find_by_fqn(&mut self, name: &str) -> sqlx::Result<Vec<ClassRecord>> {
        self.rt.block_on(async {
            sqlx::query_as::<_, ClassRecord>("SELECT * FROM fqn_declaration WHERE fqn = ?")
                .bind(name)
                .fetch_all(&mut self.conn)
                .await
        })
    }
    pub fn find_one_by_fqn(&mut self, name: &str) -> sqlx::Result<ClassRecord> {
        self.rt.block_on(async {
            sqlx::query_as::<_, ClassRecord>("SELECT * FROM fqn_declaration WHERE fqn = ?")
                .bind(name)
                .fetch_one(&mut self.conn)
                .await
        })
    }
    pub fn find_by_fqn_like(&mut self, name: &str) -> sqlx::Result<Vec<ClassRecord>> {
        self.rt.block_on(async {
            sqlx::query_as::<_, ClassRecord>("SELECT * FROM fqn_declaration WHERE fqn like ?")
                .bind(name)
                .fetch_all(&mut self.conn)
                .await
        })
    }
    pub fn find_one_by_fqn_like(&mut self, name: &str) -> sqlx::Result<ClassRecord> {
        self.rt.block_on(async {
            sqlx::query_as::<_, ClassRecord>("SELECT * FROM fqn_declaration WHERE fqn like ?")
                .bind(name)
                .fetch_one(&mut self.conn)
                .await
        })
    }
    pub fn get_class_by_method_location(
        &mut self,
        location: &Location,
    ) -> sqlx::Result<ClassRecord> {
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
            INSERT INTO fqn_declaration(
                fqn, 
                description,
                attributes,
                parameters,
                return_type,
                location_uri,
                location_position_start_line,
                location_position_start_character,
                location_position_end_line,
                location_position_end_character
            )

            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) 
            ON CONFLICT(fqn, location_uri) 
            DO UPDATE SET
                location_position_start_line = excluded.location_position_start_line,
                location_position_start_character = excluded.location_position_start_character,
                location_position_end_line = excluded.location_position_end_line,
                location_position_end_character = excluded.location_position_end_character

            "#,
            )
            .bind(symbol.fqn.clone())
            .bind(symbol.description.clone())
            .bind(symbol.attributes.clone())
            .bind(symbol.parameters.clone())
            .bind(symbol.return_type.clone())
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
