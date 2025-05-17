use moq_native::web_transport;

pub struct Connection {
	id: u64,
	session: web_transport::Session,
}

impl Connection {
	pub fn new(id: u64, session: web_transport::Session) -> Self {
		Self { id, session }
	}

	#[tracing::instrument("session", skip_all, err, fields(id = self.id))]
	pub async fn run(self, router: moq_transfork::RouterConsumer) -> anyhow::Result<()> {
		let mut session = moq_transfork::Session::accept(self.session).await?;

		session.route(router);
		session.closed().await;

		Ok(())
	}
}
