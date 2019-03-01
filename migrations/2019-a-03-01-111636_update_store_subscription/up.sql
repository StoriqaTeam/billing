ALTER TABLE store_subscription DROP COLUMN id;

ALTER TABLE store_subscription ADD PRIMARY KEY (store_id);
