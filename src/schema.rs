table! {
    accounts (id) {
        id -> Uuid,
        currency -> Text,
        is_pooled -> Bool,
        created_at -> Timestamp,
        wallet_address -> Nullable<Text>,
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
    customers (id) {
        id -> Varchar,
        user_id -> Int4,
        email -> Nullable<Varchar>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    event_store (id) {
        id -> Int8,
        event -> Jsonb,
        status -> Text,
        attempt_count -> Int4,
        created_at -> Timestamp,
        status_updated_at -> Timestamp,
    }
}

table! {
    international_billing_info (id) {
        id -> Int4,
        store_id -> Int4,
        swift_bic -> Varchar,
        bank_name -> Varchar,
        full_name -> Varchar,
        iban -> Varchar,
    }
}

table! {
    invoices (id) {
        id -> Uuid,
        invoice_id -> Uuid,
        amount -> Float8,
        price_reserved -> Timestamp,
        state -> Varchar,
        wallet -> Nullable<Varchar>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        transactions -> Jsonb,
        amount_captured -> Float8,
        currency -> Varchar,
    }
}

table! {
    invoices_v2 (id) {
        id -> Uuid,
        account_id -> Nullable<Uuid>,
        buyer_currency -> Text,
        amount_captured -> Numeric,
        final_amount_paid -> Nullable<Numeric>,
        final_cashback_amount -> Nullable<Numeric>,
        paid_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        buyer_user_id -> Int4,
        status -> Text,
    }
}

table! {
    merchants (merchant_id) {
        merchant_id -> Uuid,
        user_id -> Nullable<Int4>,
        store_id -> Nullable<Int4>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        merchant_type -> Varchar,
    }
}

table! {
    order_exchange_rates (id) {
        id -> Int8,
        order_id -> Uuid,
        exchange_id -> Nullable<Uuid>,
        exchange_rate -> Numeric,
        status -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    orders (id) {
        id -> Uuid,
        seller_currency -> Text,
        total_amount -> Numeric,
        cashback_amount -> Numeric,
        invoice_id -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        store_id -> Int4,
        state -> Varchar,
    }
}

table! {
    orders_info (id) {
        id -> Uuid,
        order_id -> Uuid,
        status -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        customer_id -> Int4,
        store_id -> Int4,
        saga_id -> Uuid,
        total_amount -> Float8,
    }
}

table! {
    payment_intent (id) {
        id -> Varchar,
        invoice_id -> Uuid,
        amount -> Numeric,
        amount_received -> Numeric,
        client_secret -> Nullable<Varchar>,
        currency -> Varchar,
        last_payment_error_message -> Nullable<Varchar>,
        receipt_email -> Nullable<Varchar>,
        charge_id -> Nullable<Varchar>,
        status -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    proxy_companies_billing_info (id) {
        id -> Int4,
        country -> Varchar,
        swift_bic -> Varchar,
        bank_name -> Varchar,
        full_name -> Varchar,
        iban -> Varchar,
    }
}

table! {
    roles (id) {
        id -> Uuid,
        user_id -> Int4,
        name -> Varchar,
        data -> Nullable<Jsonb>,
    }
}

table! {
    russia_billing_info (id) {
        id -> Int4,
        store_id -> Int4,
        kpp -> Varchar,
        bic -> Varchar,
        inn -> Varchar,
        full_name -> Varchar,
    }
}

table! {
    store_billing_type (id) {
        id -> Int4,
        store_id -> Int4,
        billing_type -> Varchar,
    }
}

joinable!(amounts_received -> invoices_v2 (invoice_id));
joinable!(invoices_v2 -> accounts (account_id));
joinable!(order_exchange_rates -> orders (order_id));
joinable!(orders -> invoices_v2 (invoice_id));
joinable!(payment_intent -> invoices_v2 (invoice_id));

allow_tables_to_appear_in_same_query!(
    accounts,
    amounts_received,
    customers,
    event_store,
    international_billing_info,
    invoices,
    invoices_v2,
    merchants,
    order_exchange_rates,
    orders,
    orders_info,
    payment_intent,
    proxy_companies_billing_info,
    roles,
    russia_billing_info,
    store_billing_type,
);
