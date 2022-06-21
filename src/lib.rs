// BP Node: bitcoin blockchain indexing and notification service
//
// Written in 2020-2022 by
//     Dr. Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2022 by LNP/BP Standards Association, Switzerland.
//
// You should have received a copy of the MIT License along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

#[macro_use]
extern crate amplify;
#[macro_use]
extern crate log;

mod config;
mod error;
pub mod bpd;
#[cfg(feature = "server")]
mod opts;

pub use config::Config;
pub use error::{DaemonError, LaunchError};
#[cfg(feature = "server")]
pub use opts::Opts;
