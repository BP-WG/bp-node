// Bitcoin transaction processing & database indexing daemon
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


use crate::config::Config as MainConfig;

#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub struct Config {
    pub db_index_url: String,
    pub db_state_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            db_index_url: String::from("postgresql://postgres:example@localhost:5432/bitcointx"),
            db_state_url: String::from("postgresql://postgres:example@localhost:5432/txparserd"),
        }
    }
}

impl From<MainConfig> for Config {
    fn from(config: MainConfig) -> Self {
        Config {
            db_index_url: config.db_index_url,
            db_state_url: config.db_state_url,
        }
    }
}
