CREATE TABLE store_billing_type
(
    id SERIAL PRIMARY KEY,
    store_id INTEGER NOT NULL,
    billing_type VARCHAR NOT NULL
);
