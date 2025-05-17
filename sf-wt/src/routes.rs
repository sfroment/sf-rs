use moq_native::{
	moq_transfork::{Router, RouterConsumer},
	quic,
};
use moq_transfork::RouterProducer;
use prost::Message;
use tracing::Instrument;

use crate::proto::KeepAliveResponse;

#[derive(Clone)]
pub struct Routes {
	client: quic::Client,

	pub router: RouterConsumer,
}

impl Routes {
	pub fn new(client: quic::Client) -> Self {
		let (producer, consumer) = Router { capacity: 1024 }.produce();

		let this = Routes {
			client,
			router: consumer,
		};

		tokio::spawn(this.clone().route_request(producer).in_current_span());

		this
	}

	async fn route_request(self, mut producer: RouterProducer) {
		while let Some(request) = producer.requested().await {
			let path = request.track.path.as_str();
			match path {
				"keep-alive" => {
					let (mut producer, consumer) = request.track.clone().produce();
					let mut group = producer.append_group();
					let mut buf = bytes::BytesMut::new();
					(KeepAliveResponse {})
						.encode(&mut buf)
						.expect("failed to encode keep alive request");
					group.write_frame(buf);
					request.serve(consumer);
				}
				_ => request.close(moq_transfork::Error::NotFound),
			}
		}
	}
}
