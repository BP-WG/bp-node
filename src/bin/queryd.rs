// Bitcoin protocol (BP) daemon node
// Written in 2020 by
//     Dr. Maxim Orlovsky <orlovsky@pandoracore.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the MIT License
// along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

#![recursion_limit = "256"]
// Coding conventions
#![deny(
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    unused_mut,
    unused_imports,
    dead_code,
    // missing_docs
)]

use clap::Parser;
use log::*;
use microservices::node::TryService;
use std::env;

use bp_node::{
    queryd::{Config, Opts, Runtime},
    BootstrapError,
};

fn main() -> Result<(), BootstrapError> {
    // TODO: Parse config file as well
    let opts: Opts = Opts::parse();
    let config: Config = opts.into();

    if env::var("RUST_LOG").is_err() {
        env::set_var(
            "RUST_LOG",
            match config.verbose {
                0 => "error",
                1 => "warn",
                2 => "info",
                3 => "debug",
                4 => "trace",
                _ => "trace",
            },
        );
    }
    env_logger::init();
    log::set_max_level(LevelFilter::Trace);

    let runtime = Runtime::init(config)?;
    runtime.run_or_panic("Queryd runtime");
    unreachable!()
}
