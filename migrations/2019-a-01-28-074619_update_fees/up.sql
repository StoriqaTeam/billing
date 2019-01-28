ALTER TABLE fees
    ADD CONSTRAINT fees_order_id_fkey
    FOREIGN KEY (order_id)
    REFERENCES orders(id)
    ON DELETE CASCADE
    ON UPDATE CASCADE;
