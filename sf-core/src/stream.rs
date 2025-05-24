use futures::{AsyncRead, AsyncWrite, future::BoxFuture};

pub trait Stream: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static {
	type Error: std::error::Error + Send + Sync + 'static;

	fn close_send(&mut self) -> BoxFuture<'_, Result<(), Self::Error>>;
	fn close_read(&mut self) -> BoxFuture<'_, Result<(), Self::Error>>;

	fn close(&mut self) -> BoxFuture<'_, Result<(), Self::Error>> {
		Box::pin(async move {
			let send_result = self.close_send().await;
			let read_result = self.close_read().await;

			send_result.and(read_result)
		})
	}
}
