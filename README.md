# billing

Microservice for managing billing.

## Running

```
cd docker && docker-compose up
```

## Request Flow

* `Application` ⇄ `Router` ⇄ `Service` ⇄ `Repo`

## Monetary values

All monetary values are stored in the database as the amount of minimal units.

Examples:
- 1 USD would be stored as 100 (100 cents)
- 1 STQ would be stored as 1000000000000000000 (1000000000000000000 wei)
