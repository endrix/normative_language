CREATE TABLE projects (
    project_id TEXT PRIMARY KEY,
    revision TEXT NOT NULL
);

CREATE TABLE sources (
    path TEXT PRIMARY KEY
);

CREATE TABLE nodes (
    uri TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(project_id),
    local_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    source_path TEXT NOT NULL REFERENCES sources(path),
    line INTEGER NOT NULL CHECK (line > 0),
    column_number INTEGER NOT NULL CHECK (column_number > 0),
    modality TEXT,
    refinement TEXT,
    domain TEXT,
    status TEXT NOT NULL,
    owner TEXT,
    body TEXT NOT NULL
);

CREATE TABLE node_attrs (
    node_uri TEXT NOT NULL REFERENCES nodes(uri) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (node_uri, key)
);

CREATE TABLE aliases (
    alias TEXT PRIMARY KEY,
    target_uri TEXT NOT NULL REFERENCES nodes(uri)
);

CREATE TABLE edges (
    edge_id INTEGER PRIMARY KEY,
    source_uri TEXT NOT NULL,
    edge_type TEXT NOT NULL,
    target_uri TEXT NOT NULL,
    source_path TEXT REFERENCES sources(path),
    line INTEGER,
    attributes_json TEXT NOT NULL
);

CREATE INDEX edges_source_idx ON edges(source_uri, edge_type, target_uri);
CREATE INDEX edges_target_idx ON edges(target_uri, edge_type, source_uri);

CREATE TABLE diagnostics (
    diagnostic_id INTEGER PRIMARY KEY,
    fingerprint TEXT NOT NULL,
    code TEXT NOT NULL,
    severity TEXT NOT NULL,
    message TEXT NOT NULL,
    subject_uri TEXT,
    source_path TEXT NOT NULL REFERENCES sources(path),
    line INTEGER NOT NULL,
    repair TEXT
);

CREATE TABLE baselines (
    fingerprint TEXT PRIMARY KEY,
    owner TEXT NOT NULL,
    expires TEXT NOT NULL
);

CREATE TABLE builds (
    build_id INTEGER PRIMARY KEY CHECK (build_id = 1),
    schema_version TEXT NOT NULL,
    semantic_hash TEXT NOT NULL,
    format_version TEXT NOT NULL,
    ir_version TEXT NOT NULL,
    tool_version TEXT NOT NULL
);
