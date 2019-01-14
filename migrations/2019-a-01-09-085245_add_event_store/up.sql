CREATE TABLE event_store
(
    id bigserial PRIMARY KEY,
    event jsonb NOT NULL,
    status text NOT NULL,
    attempt_count integer NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status_updated_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP
);
