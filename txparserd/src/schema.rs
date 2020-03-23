table! {
    cached_block (hash) {
        hash -> Bytea,
        prev_hash -> Bytea,
        block -> Bytea,
    }
}

table! {
    state (id) {
        id -> Int2,
        started_at -> Timestamp,
        updated_at -> Timestamp,
        last_block_hash -> Bytea,
        last_block_time -> Timestamp,
        known_height -> Int4,
        processed_height -> Int4,
        processed_txs -> Int8,
        processed_txins -> Int8,
        processed_txouts -> Int8,
        processed_blocks -> Int8,
        processed_volume -> Int8,
        processed_bytes -> Int8,
        processed_time -> Interval,
        utxo_size -> Int4,
        utxo_volume -> Int8,
        utxo_bytes -> Int4,
        block_cache_size -> Int4,
        block_cache_bytes -> Int4,
    }
}

table! {
    utxo (txid) {
        txid -> Bytea,
        block_height -> Int4,
        block_checksum -> Int2,
        tx_index -> Int2,
        output_index -> Int2,
    }
}

allow_tables_to_appear_in_same_query!(
    cached_block,
    state,
    utxo,
);
