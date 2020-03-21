table! {
    block (id) {
        id -> Int8,
        block_id -> Bytea,
        merkle_root -> Bytea,
        ts -> Timestamp,
        difficulty -> Int8,
        nonce -> Int4,
        ver -> Int4,
        tx_count -> Int4,
    }
}

table! {
    tx (id) {
        id -> Int8,
        ver -> Int2,
        locktime -> Int4,
        out_count -> Int4,
        in_count -> Int4,
        fee -> Nullable<Int8>,
    }
}

table! {
    txin (id) {
        id -> Int8,
        seq -> Int4,
        txout_id -> Int8,
    }
}

table! {
    txout (id) {
        id -> Int8,
        amount -> Int8,
        script -> Bytea,
    }
}

joinable!(txin -> txout (txout_id));

allow_tables_to_appear_in_same_query!(
    block,
    tx,
    txin,
    txout,
);
