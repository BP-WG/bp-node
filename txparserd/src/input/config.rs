
pub use crate::Config as MainConfig;

#[derive(Clone, PartialEq, Eq, Debug, Display)]
#[display_from(Debug)]
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

impl From<MainConfig> for Config {
    fn from(config: MainConfig) -> Self {
        Config {
            socket: config.input_socket
        }
    }
}
