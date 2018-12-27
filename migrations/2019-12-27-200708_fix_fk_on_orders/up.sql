ALTER TABLE orders DROP CONSTRAINT orders_invoice_id_fkey;

ALTER TABLE orders
    ADD CONSTRAINT orders_invoice_id_fkey
    FOREIGN KEY (invoice_id)
    REFERENCES invoices_v2(id)
    ON DELETE CASCADE
    ON UPDATE CASCADE;
