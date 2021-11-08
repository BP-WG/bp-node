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

use super::{ApiService, Config};
use crate::queryd::MonitorService;
use crate::BootstrapError;
use microservices::node::{Service, TryService};
use std::thread;

pub struct Runtime {
    config: Config,
    context: zmq::Context,
    api_service: ApiService,
    monitor_service: MonitorService,
}

impl Runtime {
    pub fn init(config: Config) -> Result<Self, BootstrapError> {
        let context = zmq::Context::new();

        let api_service = ApiService::init(config.clone().into(), context.clone())?;
        // TODO: Add push notification service
        let monitor_service = MonitorService::init(config.clone().into(), context.clone())?;

        Ok(Self {
            config,
            context,
            api_service,
            monitor_service,
        })
    }
}

impl TryService for Runtime {
    type ErrorType = BootstrapError;

    fn try_run_loop(self) -> Result<(), Self::ErrorType> {
        let api_addr = self.config.msgbus_peer_api_addr.clone();
        let monitor_addr = self.config.monitor_addr.clone();

        let api_service = self.api_service;
        let monitor_service = self.monitor_service;

        let handler = thread::spawn(move || {
            info!("API service is listening on {}", api_addr);
            api_service.run_or_panic("API service")
        });
        // TODO: Add push notification service
        thread::spawn(move || {
            info!(
                "Monitoring (Prometheus) exporter service is listening on {}",
                monitor_addr
            );
            monitor_service.run_loop()
        });

        loop {}
    }
}
