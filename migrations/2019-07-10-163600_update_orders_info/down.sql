-- This file should undo anything in `up.sql`
ALTER TABLE orders_info DROP COLUMN customer_id;
ALTER TABLE orders_info DROP COLUMN store_id;
ALTER TABLE orders_info DROP COLUMN saga_id;
