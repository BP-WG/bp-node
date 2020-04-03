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
    pub req_socket: String,
    pub pub_socket: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            req_socket: String::from("tcp://*:18318"),
            pub_socket: String::from("tcp://*:18319"),
        }
    }
}

impl From<MainConfig> for Config {
    fn from(config: MainConfig) -> Self {
        Config {
            req_socket: config.input_req_socket,
            pub_socket: config.input_pub_socket,
        }
    }
}
