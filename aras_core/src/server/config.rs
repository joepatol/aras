use std::net::IpAddr;

pub struct ServerConfig {
    pub keep_alive: bool,
    pub limit_concurrency: Option<usize>,
    pub addr: IpAddr,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            keep_alive: true,
            limit_concurrency: None,  // TODO: implement usage
            addr: [127, 0, 0, 1].into(),
            port: 8080,
        }
    }
}