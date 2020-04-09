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


use lnpbp::service::*;

use super::*;
use crate::error::BootstrapError;


pub struct Runtime {
    config: Config,
}

impl Runtime {
    pub async fn init(config: Config) -> Result<Self, BootstrapError> {
        // TODO: Check directory
        // TODO: Connect to db

        debug!("Indexer is launched");
        Ok(Self {
            config,
        })
    }
}

#[async_trait]
impl TryService for Runtime {
    type ErrorType = tokio::task::JoinError;

    async fn try_run_loop(self) -> Result<!, Self::ErrorType> {
        loop {

        }
    }
}
