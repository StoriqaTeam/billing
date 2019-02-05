CREATE TABLE user_wallets (
    id uuid PRIMARY KEY,
    address text NOT NULL,
    currency text NOT NULL,
    user_id integer NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP
);
