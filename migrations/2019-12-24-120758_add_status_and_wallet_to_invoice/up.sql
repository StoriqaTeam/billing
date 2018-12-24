ALTER TABLE invoices_v2 ADD COLUMN status text NOT NULL DEFAULT 'new';
ALTER TABLE invoices_v2 ADD COLUMN wallet_address text;
