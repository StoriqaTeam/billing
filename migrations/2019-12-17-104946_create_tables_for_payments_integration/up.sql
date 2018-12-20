CREATE TABLE accounts
(
    id uuid NOT NULL,
    currency text NOT NULL,
    is_pooled boolean NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT accounts_pkey PRIMARY KEY (id)
);

CREATE TABLE invoices_v2
(
    id uuid NOT NULL,
    account_id uuid,
    buyer_currency text NOT NULL,
    amount_captured bigint NOT NULL,
    final_amount_paid bigint,
    final_cashback_amount bigint,
    paid_at timestamp without time zone,
    created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT invoices_v2_pkey PRIMARY KEY (id),
    CONSTRAINT invoices_v2_account_id_unique_idx UNIQUE (account_id),
    CONSTRAINT invoices_v2_account_id_fkey FOREIGN KEY (account_id)
        REFERENCES accounts (id) MATCH SIMPLE
        ON UPDATE CASCADE
        ON DELETE SET NULL
);

SELECT diesel_manage_updated_at('invoices_v2');

CREATE TABLE orders
(
    id uuid NOT NULL,
    seller_currency text NOT NULL,
    total_amount bigint NOT NULL,
    cashback_amount bigint NOT NULL,
    invoice_id uuid NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT orders_pkey PRIMARY KEY (id),
    CONSTRAINT orders_invoice_id_fkey FOREIGN KEY (id)
        REFERENCES invoices_v2 (id) MATCH SIMPLE
        ON UPDATE CASCADE
        ON DELETE CASCADE
);

SELECT diesel_manage_updated_at('orders');

CREATE TABLE order_exchange_rates
(
    id serial NOT NULL,
    order_id uuid NOT NULL,
    exchange_id uuid,
    exchange_rate numeric NOT NULL,
    status text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT order_exchange_rates_pkey PRIMARY KEY (id),
    CONSTRAINT order_exchange_rates_order_id_fkey FOREIGN KEY (order_id)
        REFERENCES orders (id) MATCH SIMPLE
        ON UPDATE CASCADE
        ON DELETE CASCADE
);

SELECT diesel_manage_updated_at('order_exchange_rates');
