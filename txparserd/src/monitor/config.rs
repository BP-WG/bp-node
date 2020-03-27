
pub use crate::controller;

#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
pub struct Config {
    pub socket: String
}

impl Default for Config {
    fn default() -> Self {
        Self {
            socket: String::from("tcp://0.0.0.0:14224")
        }
    }
}

impl From<controller::Config> for Config {
    fn from(config: controller::Config) -> Self {
        Config {
            socket: config.input_socket
        }
    }
}
