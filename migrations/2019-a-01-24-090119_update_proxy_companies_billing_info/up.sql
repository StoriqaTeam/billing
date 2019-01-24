DROP TABLE proxy_companies_billing_info;

CREATE TABLE proxy_companies_billing_info
(
    id SERIAL PRIMARY KEY,
    country_alpha3 VARCHAR NOT NULL,
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
