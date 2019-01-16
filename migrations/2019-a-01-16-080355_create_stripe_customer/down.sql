DROP TABLE customers;

-- fixed payment_intent table, diesel work only table with primary key

ALTER TABLE payment_intent
DROP CONSTRAINT payment_intent_pkey;

-- fixed merchants table, not equal coloumn name in sql table `type` in models `merchant_type`

ALTER TABLE merchants ADD COLUMN type VARCHAR NOT NULL DEFAULT 'store';
UPDATE merchants SET type = merchant_type;
ALTER TABLE merchants DROP COLUMN merchant_type;

-- fixed invoices table, invoice_id NOT NULL

ALTER TABLE invoices
ALTER COLUMN invoice_id DROP NOT NULL;