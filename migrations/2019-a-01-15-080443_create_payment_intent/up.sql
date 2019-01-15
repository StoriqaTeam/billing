CREATE TABLE payment_intent
(
    id VARCHAR NOT NULL UNIQUE,
    invoice_id UUID NOT NULL REFERENCES invoices_v2 (id),
    amount NUMERIC NOT NULL,
    amount_received NUMERIC NOT NULL,
    client_secret VARCHAR,
    currency VARCHAR NOT NULL,
    last_payment_error_message VARCHAR,
    receipt_email VARCHAR,
    charge_id VARCHAR,
    status VARCHAR NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP
);
