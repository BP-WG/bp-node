-- Your SQL goes here

create table block
(
    id          bigint    not null
        constraint block_pk
            primary key,
    block_id    bytea     not null,
    merkle_root bytea     not null,
    ts          timestamp not null,
    difficulty  bigint    not null,
    nonce       integer   not null,
    ver         integer   not null,
    tx_count    integer   not null
);

create unique index block_blockid_uindex
    on block (block_id);

create unique index block_id_uindex
    on block (id);

create index block_ts_index
    on block (ts);

create index block_txcount_index
    on block (tx_count);

create index block_ver_index
    on block (ver);

create table tx
(
    id        bigint   not null
        constraint tx_pk
            primary key,
    ver       integer  not null,
    locktime  integer  not null,
    out_count smallint not null,
    in_count  smallint not null,
    fee       bigint
);

create unique index tx_id_uindex
    on tx (id);

create table txout
(
    id     bigint not null
        constraint txout_pk
            primary key,
    amount bigint not null,
    script bytea  not null
);

create index txout_amount_index
    on txout (amount);

create unique index txout_id_uindex
    on txout (id);

create index txout_script_index
    on txout (script);

create table txin
(
    id       bigint  not null
        constraint txin_pk
            primary key,
    seq         integer not null,
    txout_id bigint  not null
        constraint txin_txout_id_fk
            references txout
            on update cascade on delete cascade
);

create unique index txin_id_uindex
    on txin (id);

create index txin_seq_index
    on txin (seq);
