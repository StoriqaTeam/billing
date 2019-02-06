ALTER TABLE user_wallets ADD COLUMN is_active boolean NOT NULL;

CREATE TABLE payouts (
    order_id uuid PRIMARY KEY,
    amount numeric NOT NULL,
    completed_at timestamp without time zone NOT NULL,
    payout_target_type text NOT NULL,
    user_wallet_id uuid NULL,
    blockchain_fee numeric NULL,

    CONSTRAINT payouts_order_id_fkey FOREIGN KEY (order_id)
        REFERENCES orders (id)
        ON UPDATE CASCADE
        ON DELETE SET NULL,

    CONSTRAINT payouts_user_wallet_id_fkey FOREIGN KEY (user_wallet_id)
        REFERENCES user_wallets (id)
        ON UPDATE CASCADE
        ON DELETE SET NULL
);
