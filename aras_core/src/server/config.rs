use std::net::IpAddr;

use tokio::sync::Semaphore;

pub struct ServerConfig {
    pub keep_alive: bool,
    pub limit_concurrency: usize,
    pub addr: IpAddr,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            keep_alive: true,
            limit_concurrency: Semaphore::MAX_PERMITS,  // TODO: implement usage
            addr: [127, 0, 0, 1].into(),
            port: 8083,
        }
    }
}