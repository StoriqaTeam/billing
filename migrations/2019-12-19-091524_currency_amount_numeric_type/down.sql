ALTER TABLE invoices_v2 ALTER COLUMN amount_captured TYPE bigint;
ALTER TABLE invoices_v2 ALTER COLUMN final_amount_paid TYPE bigint;
ALTER TABLE invoices_v2 ALTER COLUMN final_cashback_amount TYPE bigint;

ALTER TABLE orders ALTER COLUMN total_amount TYPE bigint;
ALTER TABLE orders ALTER COLUMN cashback_amount TYPE bigint;
