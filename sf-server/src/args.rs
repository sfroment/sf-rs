use clap::Parser;
use std::net::SocketAddr;

#[derive(Parser, Debug)]
#[clap(
    name = "sf-ice",
    rename_all = "kebab-case",
    rename_all_env = "screaming-snake"
)]
pub(crate) struct Args {
    #[clap(short = 'a', long, default_value = "0.0.0.0:9999", env)]
    pub(crate) host: SocketAddr,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::{
        env,
        net::{IpAddr, Ipv4Addr},
    };

    #[test]
    #[serial(env)]
    fn test_default_host() {
        let args = Args::parse_from::<_, &str>([]);
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9999);
        assert_eq!(args.host, socket);
    }

    #[test]
    #[serial(env)]
    fn test_custom_host_long() {
        let args = Args::parse_from(["sf-ice", "--host", "127.0.0.1:8080"]);
        assert_eq!(args.host.to_string(), "127.0.0.1:8080");
    }

    #[test]
    #[serial(env)]
    fn test_custom_host_short() {
        let args = Args::parse_from(["sf-ice", "-a", "127.0.0.1:8080"]);
        assert_eq!(args.host.to_string(), "127.0.0.1:8080");
    }

    #[test]
    #[serial(env)]
    fn test_host_from_env() {
        unsafe {
            env::set_var("HOST", "127.0.0.1:9090");
            let args = Args::parse_from::<_, &str>([]);
            assert_eq!(args.host.to_string(), "127.0.0.1:9090");
            env::remove_var("HOST");
        }
    }
}
