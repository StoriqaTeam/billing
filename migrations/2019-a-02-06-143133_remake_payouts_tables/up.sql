DROP TABLE IF EXISTS payouts;

CREATE TABLE payouts (
    id uuid PRIMARY KEY,
    currency text NOT NULL,
    gross_amount numeric NOT NULL,
    net_amount numeric NOT NULL,
    user_id integer NOT NULL,
    initiated_at timestamp without time zone NOT NULL,
    completed_at timestamp without time zone NULL,
    payout_target_type text NOT NULL,
    wallet_address text NULL,
    blockchain_fee numeric NULL
);

CREATE TABLE order_payouts (
    id bigserial PRIMARY KEY,
    order_id uuid NOT NULL,
    payout_id uuid NOT NULL,

    UNIQUE (order_id, payout_id),

    CONSTRAINT order_payouts_order_id_fkey FOREIGN KEY (order_id)
        REFERENCES orders (id)
        ON UPDATE CASCADE
        ON DELETE CASCADE,

    CONSTRAINT order_payouts_payout_id_fkey FOREIGN KEY (payout_id)
        REFERENCES payouts (id)
        ON UPDATE CASCADE
        ON DELETE CASCADE
);
