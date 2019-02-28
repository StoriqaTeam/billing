CREATE TABLE store_subscription (
    id SERIAL PRIMARY KEY,
    store_id INTEGER NOT NULL,
    currency VARCHAR NOT NULL,
    value NUMERIC NOT NULL,
    wallet_address VARCHAR,
    trial_start_date timestamp without time zone,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);

CREATE TABLE subscription_payment (
    id SERIAL PRIMARY KEY,
    store_id INTEGER NOT NULL,
    amount NUMERIC NOT NULL,
    currency VARCHAR NOT NULL,
    charge_id VARCHAR, 
    transaction_id UUID,
    status VARCHAR NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE subscription (
    id SERIAL PRIMARY KEY,
    store_id INTEGER NOT NULL,
    published_base_products_quantity INTEGER NOT NULL,
    subscription_payment_id INTEGER REFERENCES subscription_payment (id),
    created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP
);
