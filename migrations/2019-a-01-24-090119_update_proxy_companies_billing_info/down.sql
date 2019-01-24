DROP TABLE proxy_companies_billing_info;

CREATE TABLE proxy_companies_billing_info
(
    id SERIAL PRIMARY KEY,
    country VARCHAR NOT NULL,
    swift_bic VARCHAR NOT NULL,
    bank_name VARCHAR NOT NULL,
    full_name VARCHAR NOT NULL,
    iban VARCHAR NOT NULL
);
