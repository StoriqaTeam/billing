-- Your SQL goes here
CREATE TABLE merchants (
    merchant_id UUID PRIMARY KEY,
    user_id INTEGER,
    store_id INTEGER,
    type VARCHAR NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);

SELECT diesel_manage_updated_at('merchants');
