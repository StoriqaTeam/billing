-- This file should undo anything in `up.sql`
ALTER TABLE order_info DROP COLUMN customer_id;
ALTER TABLE order_info DROP COLUMN store_id;
