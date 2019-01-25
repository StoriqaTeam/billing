ALTER TABLE payment_intent ADD COLUMN invoice_id UUID NOT NULL REFERENCES invoices_v2 (id);

UPDATE payment_intent SET invoice_id = a.invoice_id
FROM payment_intents_invoices as a
WHERE payment_intent.id = a.payment_intent_id;

DROP TABLE IF EXISTS payment_intents_invoices;
