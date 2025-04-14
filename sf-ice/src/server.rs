use axum::Router;
use axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use std::net::{SocketAddr, TcpListener};
use tracing::info;

pub struct Server {
    pub local_addr: Option<SocketAddr>,

    addr: SocketAddr,

    svc_info: IntoMakeServiceWithConnectInfo<Router, SocketAddr>,

    listener: Option<TcpListener>,
}

impl Server {
    pub fn new(
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

    fn bind(&mut self) -> Result<SocketAddr, crate::Error> {
        let listener = TcpListener::bind(self.addr).map_err(crate::Error::Bind)?;
        listener.set_nonblocking(true).map_err(crate::Error::Bind)?;
        self.local_addr = Some(listener.local_addr().unwrap());
        self.listener = Some(listener);
        Ok(self.local_addr.unwrap())
    }

    fn unit_bind(&mut self) -> Result<(), crate::Error> {
        match self.listener {
            Some(_) => Ok(()),
            None => self.bind().map(drop),
        }
    }

    /// Serve the server
    pub async fn serve(mut self) -> Result<(), crate::Error> {
        self.unit_bind()?;
        let listener = tokio::net::TcpListener::from_std(self.listener.expect("No listener"))
            .map_err(crate::Error::Bind)?;

        info!("Server started on {}", self.local_addr.unwrap());

        axum::serve(listener, self.svc_info)
            .await
            .map_err(crate::Error::Serve)
    }
}
