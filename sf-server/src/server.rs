use axum::{Router, extract::connect_info::IntoMakeServiceWithConnectInfo};
use std::net::{SocketAddr, TcpListener};
use tracing::info;

pub struct Server {
    pub local_addr: Option<SocketAddr>,

    pub(crate) addr: SocketAddr,

    svc_info: IntoMakeServiceWithConnectInfo<Router, SocketAddr>,

    listener: Option<TcpListener>,
}

impl Server {
    pub(crate) fn new(
        addr: impl Into<SocketAddr>,
        svc_info: IntoMakeServiceWithConnectInfo<Router, SocketAddr>,
    ) -> Self {
        Self {
            addr: addr.into(),
            svc_info,
            listener: None,
            local_addr: None,
        }
    }

    fn bind(&mut self) -> Result<SocketAddr, crate::error::Error> {
        let listener = TcpListener::bind(self.addr).map_err(crate::error::Error::Bind)?;
        listener
            .set_nonblocking(true)
            .map_err(crate::error::Error::Bind)?;
        self.local_addr = Some(listener.local_addr().unwrap());
        self.listener = Some(listener);
        Ok(self.local_addr.unwrap())
    }

    fn unit_bind(&mut self) -> Result<(), crate::error::Error> {
        match self.listener {
            Some(_) => Ok(()),
            None => self.bind().map(drop),
        }
    }

    /// Serve the server
    pub async fn serve(mut self) -> Result<(), crate::error::Error> {
        self.unit_bind()?;
        let listener = tokio::net::TcpListener::from_std(self.listener.expect("No listener"))
            .map_err(crate::error::Error::Bind)?;

        info!("Server started on {}", self.local_addr.unwrap());

        axum::serve(listener, self.svc_info)
            .await
            .map_err(crate::error::Error::Serve)
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let addr = "127.0.0.1:8080".parse::<SocketAddr>().unwrap();
        let router = Router::new().into_make_service_with_connect_info::<SocketAddr>();

        let server = Server::new(addr, router);

        assert_eq!(server.addr, addr);
        assert!(server.listener.is_none());
        assert!(server.local_addr.is_none());
    }

    #[test]
    fn test_bind() {
        let addr = "127.0.0.1:8081".parse::<SocketAddr>().unwrap();
        let router = Router::new().into_make_service_with_connect_info::<SocketAddr>();

        let mut server = Server::new(addr, router);

        let bound_addr = server.bind().expect("Failed to bind server");
        assert!(server.listener.is_some());
        assert_eq!(server.local_addr, Some(bound_addr));
        assert_ne!(
            bound_addr.port(),
            0,
            "Port should be assigned after binding"
        );
        let bound_addr = server.bind();
        assert!(matches!(bound_addr, Err(crate::error::Error::Bind(_))));
    }

    #[test]
    fn test_unit_bind() {
        let addr = "127.0.0.1:0".parse::<SocketAddr>().unwrap();
        let router = Router::new().into_make_service_with_connect_info::<SocketAddr>();

        let mut server = Server::new(addr, router);

        server.unit_bind().expect("Failed to bind server");
        assert!(server.listener.is_some());
        server.unit_bind().expect("Failed to bind server");
    }

    #[tokio::test]
    async fn test_serve() {
        let addr = "127.0.0.1:0".parse::<SocketAddr>().unwrap();
        let router = Router::new().into_make_service_with_connect_info::<SocketAddr>();

        let mut server = Server::new(addr, router);

        let bound_addr = server.bind().expect("Failed to bind server");

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();

        let test_addr = bound_addr;

        let server_handle = tokio::spawn(async move {
            let server_fut = server.serve();
            tokio::select! {
                res = server_fut => res,
                _ = rx => Ok(())
            }
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let tcp_stream = tokio::net::TcpStream::connect(test_addr).await;
        assert!(tcp_stream.is_ok(), "Failed to connect to server");

        // Shutdown the server
        let _ = tx.send(());
        let _ = server_handle.await;
    }
}
