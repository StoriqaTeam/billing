CREATE TABLE payment_intents_invoices (
    id SERIAL PRIMARY KEY,
    invoice_id UUID NOT NULL REFERENCES invoices_v2 (id),
    payment_intent_id VARCHAR NOT NULL REFERENCES payment_intent (id),
    created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS payment_intents_invoices_invoice_id_payment_intent_id_unique_idx ON payment_intents_invoices (invoice_id, payment_intent_id);

INSERT INTO payment_intents_invoices (invoice_id, payment_intent_id)
SELECT DISTINCT invoice_id, id 
FROM payment_intent
ON CONFLICT DO NOTHING;

ALTER TABLE payment_intent DROP COLUMN invoice_id;