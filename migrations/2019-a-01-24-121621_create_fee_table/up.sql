CREATE TABLE fees
(
    id SERIAL PRIMARY KEY,
    order_id UUID NOT NULL,
    amount NUMERIC NOT NULL,
    status VARCHAR NOT NULL,
    currency VARCHAR NOT NULL,
    charge_id VARCHAR,
    metadata JSONB,
    created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP
);
