ALTER TABLE store_subscription DROP CONSTRAINT store_subscription_pkey;

ALTER TABLE store_subscription ADD COLUMN id SERIAL PRIMARY KEY;
