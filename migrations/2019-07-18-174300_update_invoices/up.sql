ALTER TABLE invoices DROP COLUMN transaction_id;
ALTER TABLE invoices DROP COLUMN transaction_captured_amount;
ALTER TABLE invoices ADD COLUMN transactions JSONB NOT NULL DEFAULT '[]';
