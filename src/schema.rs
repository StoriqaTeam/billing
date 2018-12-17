table! {
    invoices (id) {
        id -> Uuid,
        invoice_id -> Uuid,
        transactions -> Jsonb,
        amount -> Double,
        currency -> VarChar,
        price_reserved -> Timestamp, // UTC 0, generated at db level
        state -> VarChar,
        wallet -> Nullable<VarChar>,
        amount_captured -> Double,
    }
}

table! {
    merchants (merchant_id) {
        merchant_id -> Uuid,
        user_id -> Nullable<Integer>,
        store_id -> Nullable<Integer>,
        #[sql_name = "type"]
        merchant_type -> VarChar,
    }
}

table! {
    orders_info (id) {
        id -> Uuid,
        order_id -> Uuid,
        store_id -> Integer,
        customer_id -> Integer,
        saga_id -> Uuid,
        status -> VarChar,
        total_amount -> Double,
    }
}

table! {
    roles (id) {
        id -> Uuid,
        user_id -> Integer,
        name -> VarChar,
        data -> Nullable<Jsonb>,
    }
}

table! {
    accounts (id) {
        id -> Uuid,
        currency -> VarChar,
        is_pooled -> Bool,
        created_at -> Timestamp,
    }
}

table! {
    invoices_v2 (id) {
        id -> Uuid,
        account_id -> Nullable<Uuid>,
        buyer_currency -> VarChar,
        amount_captured -> Int8,
        final_amount_paid -> Nullable<Int8>,
        final_cashback_amount -> Nullable<Int8>,
        paid_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    orders (id) {
        id -> Uuid,
        seller_currency -> VarChar,
        total_amount -> Int8,
        cashback_amount -> Int8,
        invoice_id -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    order_exchange_rates (id) {
        id -> Int4,
        order_id -> Uuid,
        exchange_id -> Nullable<Uuid>,
        exchange_rate -> Numeric,
        status -> VarChar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}
