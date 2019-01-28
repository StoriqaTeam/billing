DROP TABLE russia_billing_info;

CREATE TABLE russia_billing_info
(
    id SERIAL PRIMARY KEY,
    store_id INTEGER NOT NULL,
    bank_name VARCHAR NOT NULL,
    branch_name VARCHAR,
    swift_bic VARCHAR NOT NULL,
    tax_id VARCHAR NOT NULL,
    correspondent_account VARCHAR NOT NULL,
    current_account VARCHAR NOT NULL,
    personal_account VARCHAR,
    beneficiary_full_name VARCHAR NOT NULL
);

DROP TABLE international_billing_info;

CREATE TABLE international_billing_info
(
    id SERIAL PRIMARY KEY,
    store_id INTEGER NOT NULL,
    account VARCHAR NOT NULL,
    currency VARCHAR NOT NULL,
    name VARCHAR NOT NULL,
    bank VARCHAR NOT NULL,
    swift VARCHAR NOT NULL,
    bank_address VARCHAR NOT NULL,
    country VARCHAR NOT NULL,
    city VARCHAR NOT NULL,
    recipient_address VARCHAR NOT NULL
);
