-- Your SQL goes here
CREATE TABLE invoices (
    id UUID PRIMARY KEY,
    invoice_id UUID,
    billing_url VARCHAR NOT NULL,
    transaction_id VARCHAR,
    transaction_captured_amount DOUBLE PRECISION,
    amount DOUBLE PRECISION NOT NULL,
    currency_id INTEGER NOT NULL,
    price_reserved TIMESTAMP NOT NULL,
    state VARCHAR NOT NULL,
    wallet VARCHAR NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);

SELECT diesel_manage_updated_at('invoices');
