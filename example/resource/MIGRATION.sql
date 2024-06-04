CREATE TABLE IF NOT EXISTS events (
    id UUID PRIMARY KEY,
    entity_id TEXT NOT NULL,
    seq_nr BIGINT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    payload JSONB NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS events_entity_id_seq_nr_idx ON events (entity_id, seq_nr);
