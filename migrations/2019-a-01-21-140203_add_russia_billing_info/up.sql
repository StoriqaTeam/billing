CREATE TABLE russia_billing_info
(
    id SERIAL PRIMARY KEY,
    store_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    kpp VARCHAR,
    bic VARCHAR
);
