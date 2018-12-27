ALTER TABLE invoices_v2 ADD COLUMN status text NOT NULL DEFAULT 'new';
ALTER TABLE orders ADD COLUMN store_id integer NOT NULL;
ALTER TABLE accounts ADD COLUMN wallet_address text;
