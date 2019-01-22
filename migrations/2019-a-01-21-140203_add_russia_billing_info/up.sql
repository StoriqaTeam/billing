CREATE TABLE russia_billing_info
(
    id SERIAL PRIMARY KEY,
    store_id INTEGER NOT NULL,
    kpp VARCHAR NOT NULL,
    bic VARCHAR NOT NULL,
    inn VARCHAR NOT NULL,
    full_name VARCHAR NOT NULL
);
