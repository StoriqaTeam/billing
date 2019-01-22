CREATE TABLE international_billing_info
(
    id SERIAL PRIMARY KEY,
    store_id INTEGER NOT NULL,
    swift_bic VARCHAR NOT NULL,
    bank_name VARCHAR NOT NULL,
    full_name VARCHAR NOT NULL,
    iban VARCHAR NOT NULL
);
