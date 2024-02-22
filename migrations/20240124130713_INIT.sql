-- Add migration script here
CREATE TABLE IF NOT EXISTS symbol(
    id INTEGER NOT NULL PRIMARY KEY,
    kind INTEGER,
    name TEXT,

    fqn TEXT, 
    implements TEXT,
    implementations TEXT,

    location_uri TEXT,
    location_position_start_line INTEGER,
    location_position_start_character INTEGER,
    location_position_end_line INTEGER,
    location_position_end_character INTEGER
);
CREATE UNIQUE INDEX unique_kind_name_loc
ON symbol(kind,fqn,location_uri);

CREATE TABLE IF NOT EXISTS namespace_map(
    id INTEGER NOT NULL PRIMARY KEY,
    symbol_id INTEGER NOT NULL ,
    name TEXT,
    alias TEXT,
    location_uri TEXT,
    location_position_start_line INTEGER,
    location_position_start_character INTEGER,
    location_position_end_line INTEGER,
    location_position_end_character INTEGER
);
