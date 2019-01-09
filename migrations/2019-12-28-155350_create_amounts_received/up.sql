CREATE TABLE amounts_received
(
    id uuid NOT NULL,
    invoice_id uuid NOT NULL,
    amount_received numeric NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT amounts_received_pkey PRIMARY KEY (id),
    CONSTRAINT amounts_captured_invoice_id_fkey FOREIGN KEY (invoice_id)
        REFERENCES invoices_v2 (id)
        ON UPDATE CASCADE
        ON DELETE CASCADE
);
