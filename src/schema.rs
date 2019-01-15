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
        wallet_address -> Nullable<VarChar>,
    }
}

table! {
    invoices_v2 (id) {
        id -> Uuid,
        account_id -> Nullable<Uuid>,
        buyer_currency -> VarChar,
        amount_captured -> Numeric,
        final_amount_paid -> Nullable<Numeric>,
        final_cashback_amount -> Nullable<Numeric>,
        paid_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        buyer_user_id -> Integer,
        status -> VarChar,
    }
}

table! {
    orders (id) {
        id -> Uuid,
        seller_currency -> VarChar,
        total_amount -> Numeric,
        cashback_amount -> Numeric,
        invoice_id -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        store_id -> Integer,
    }
}

table! {
    order_exchange_rates (id) {
        id -> Int8,
        order_id -> Uuid,
        exchange_id -> Nullable<Uuid>,
        exchange_rate -> Numeric,
        status -> VarChar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    amounts_received (id) {
        id -> Uuid,
        invoice_id -> Uuid,
        amount_received -> Numeric,
        created_at -> Timestamp,
    }
}

table! {
    event_store (id) {
        id -> Int8,
        event -> Jsonb,
        status -> VarChar,
        attempt_count -> Int4,
        created_at -> Timestamp,
        status_updated_at -> Timestamp,
    }
}

table! {
    payment_intent (id) {
        id -> VarChar,
        invoice_id -> Uuid,
        amount -> Numeric,
        amount_received -> Numeric,
        client_secret -> Nullable<VarChar>,
        currency -> VarChar,
        last_payment_error_message -> Nullable<VarChar>,
        receipt_email -> Nullable<VarChar>,
        charge_id -> Nullable<VarChar>,
        status -> VarChar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

joinable!(payment_intent -> invoices_v2 (invoice_id));
joinable!(invoices_v2 -> accounts (account_id));
joinable!(orders -> invoices_v2 (invoice_id));
joinable!(order_exchange_rates -> orders (order_id));
joinable!(amounts_received -> invoices_v2 (invoice_id));
allow_tables_to_appear_in_same_query!(accounts, invoices_v2, orders, order_exchange_rates);
