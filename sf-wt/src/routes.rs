use moq_native::{
	moq_transfork::{Router, RouterConsumer},
	quic,
};
use moq_transfork::{GroupProducer, RouterProducer, Track, TrackProducer};
use tracing::{Instrument, info};

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
			info!("path: {:?}", path);
			match path {
				"test" => {
					let (mut producer, consumer) = request.track.clone().produce();
					let mut group = producer.append_group();
					group.write_frame("TEST");
					request.serve(consumer);
				}
				_ => request.close(moq_transfork::Error::NotFound),
			}
		}
	}
}
