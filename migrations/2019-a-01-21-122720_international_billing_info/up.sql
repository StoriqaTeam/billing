CREATE TABLE international_billing_info
(
    id SERIAL PRIMARY KEY,
    store_id INTEGER NOT NULL,
    swift_bic VARCHAR,
    bank_name VARCHAR,
    full_name VARCHAR,
    iban VARCHAR
);
