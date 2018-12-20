ALTER TABLE invoices_v2 ALTER COLUMN amount_captured TYPE numeric;
ALTER TABLE invoices_v2 ALTER COLUMN final_amount_paid TYPE numeric;
ALTER TABLE invoices_v2 ALTER COLUMN final_cashback_amount TYPE numeric;

ALTER TABLE orders ALTER COLUMN total_amount TYPE numeric;
ALTER TABLE orders ALTER COLUMN cashback_amount TYPE numeric;
