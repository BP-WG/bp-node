
pub use crate::controller;

pub struct Config {
    pub socket: String
}

impl Default for Config {
    fn default() -> Self {
        Self {
            socket: String::from("tcp://0.0.0.0:88318")
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
