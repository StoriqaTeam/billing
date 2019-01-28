CREATE TABLE payment_intents_fees (
    id SERIAL PRIMARY KEY,
    fee_id INTEGER NOT NULL REFERENCES fees (id),
    payment_intent_id VARCHAR NOT NULL REFERENCES payment_intent (id),
    created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS payment_intents_fees_fee_id_payment_intent_id_unique_idx ON payment_intents_fees (fee_id, payment_intent_id);
