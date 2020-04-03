-- Your SQL goes here

create user txparserd password 'txparserd';

create table state
(
    id smallserial  not null
        constraint state_pk
            primary key,

    started_at timestamp not null,
    updated_at timestamp not null,

    last_block_hash bytea not null,
    last_block_time timestamp not null,

    known_height integer not null default 0,
    processed_height integer not null default 0,
    processed_txs bigint not null default 0,
    processed_txins bigint not null default 0,
    processed_txouts bigint not null default 0,
    processed_blocks bigint not null default 0,
    processed_volume bigint not null default 0,
    processed_bytes bigint not null default 0,
    processed_time interval not null,

    utxo_size integer not null default 0,
    utxo_volume bigint not null default 0,
    utxo_bytes integer not null default 0,

    block_cache_size integer not null default 0,
    block_cache_bytes integer not null default 0
);

alter table state
    owner to txparserd;

create table cached_block
(
    hash bytea not null
        constraint cached_block_id
            primary key,
    prev_hash bytea not null,
    block bytea not null
);

alter table cached_block
    owner to txparserd;

create table utxo
(
    txid bytea not null,
    block_height integer not null,
    block_checksum smallint not null,
    tx_index smallint not null,
    output_index smallint not null,
    constraint utxo_id
        primary key (txid, output_index)
);

alter table utxo
    owner to txparserd;
