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

use crate::{input, monitor, parser};

#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub struct Config {
    pub input_socket: String,
    pub monitor_socket: String,
    pub db_index_url: String,
    pub db_state_url: String,
}

impl Default for Config {
    fn default() -> Self {
        let input_config = input::Config::default();
        let monitor_config = monitor::Config::default();
        let parser_config = parser::Config::default();
        Self {
            input_socket: input_config.socket,
            monitor_socket: monitor_config.socket,
            db_index_url: parser_config.db_index_url,
            db_state_url: parser_config.db_state_url,
        }
    }
}
