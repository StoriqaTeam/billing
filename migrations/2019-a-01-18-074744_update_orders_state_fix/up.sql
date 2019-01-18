UPDATE orders SET state = 'initial' where state = 'initital';
ALTER TABLE orders ALTER COLUMN state DROP DEFAULT; 
ALTER TABLE orders ALTER COLUMN state SET DEFAULT 'initial';
