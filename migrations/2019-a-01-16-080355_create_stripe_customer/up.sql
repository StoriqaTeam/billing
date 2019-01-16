CREATE TABLE customers
(
    id VARCHAR NOT NULL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    email VARCHAR,   
    created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP
);


-- fixed payment_intent table, diesel work only table with primary key

ALTER TABLE payment_intent
ADD PRIMARY KEY (id);

-- fixed merchants table, not equal coloumn name in sql table `type` in models `merchant_type`

ALTER TABLE merchants ADD COLUMN merchant_type VARCHAR NOT NULL DEFAULT 'store';
UPDATE merchants SET merchant_type = type;
ALTER TABLE merchants DROP COLUMN type;

-- fixed invoices table, invoice_id NOT NULL

ALTER TABLE invoices
ALTER COLUMN invoice_id SET NOT NULL;