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

// We need this since code is not completed and a lot of it is written
// for future functionality
// Remove this once the first version will be complete
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(unreachable_patterns)]
// In mutithread environments it's critical to capture all failures
#![deny(unused_must_use)]
#![feature(unwrap_infallible)]
#![feature(in_band_lifetimes)]

extern crate zmq;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate derive_wrapper;
extern crate chrono;
extern crate dotenv;
extern crate env_logger;
#[macro_use]
extern crate log;
extern crate prometheus;
extern crate tiny_http;

pub mod error;
pub mod msgbus;
pub mod queryd;
pub mod util;

pub mod cli;
pub mod indexer;

pub mod db;
pub mod parser;

pub use error::*;
