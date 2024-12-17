use std::net::IpAddr;

use tokio::sync::Semaphore;

pub struct ServerConfig {
    pub keep_alive: bool,
    pub limit_concurrency: usize,
    pub addr: IpAddr,
    pub port: u16,
}

impl ServerConfig {
    pub fn new(
        keep_alive: bool,
        max_concurrency: Option<usize>,
        addr: IpAddr,
        port: u16,
    ) -> Self {
        Self { 
            keep_alive,
            limit_concurrency: max_concurrency.unwrap_or(Semaphore::MAX_PERMITS) ,
            addr,
            port,
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            keep_alive: true,
            limit_concurrency: Semaphore::MAX_PERMITS,
            addr: [127, 0, 0, 1].into(),
            port: 8083,
        }
    }
}