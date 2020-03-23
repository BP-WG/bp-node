-- This file should undo anything in `up.sql`

drop table state;
drop table cached_block;
drop table utxo;

drop user txparserd;
