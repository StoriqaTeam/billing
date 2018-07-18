ALTER TABLE orders_info DROP COLUMN saga_id;
ALTER TABLE orders_info ADD COLUMN saga_id UUID NOT NULL DEFAULT uuid_generate_v4();
