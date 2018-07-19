ALTER TABLE invoices DROP COLUMN transactions;
ALTER TABLE invoices ADD COLUMN transaction_id VARCHAR;
ALTER TABLE invoices ADD COLUMN transaction_captured_amount DOUBLE PRECISION;
