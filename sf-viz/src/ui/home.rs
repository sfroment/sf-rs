use dioxus::prelude::*;
use tracing::info;

use crate::wt::{self, ClientContext};

/// Home page
#[component]
pub fn Home() -> Element {
	rsx! {
		div { id: "home", ConnectionPanel {} }
	}
}

#[component]
fn ConnectionPanel() -> Element {
	let mut input_value = use_signal(|| "https://localhost:4433".to_string());

	rsx! {
		div { class: "connection-panel",
			div { id: "peer-id", "Your Peer ID" }
			div { class: "input-group",
				input {
					placeholder: "https://localhost:4433",
					r#type: "url",
					value: "{input_value}",
					oninput: move |event| input_value.set(event.value()),
					onkeypress: move |event| {
						if event.key() == Key::Enter {
							event.prevent_default();
							spawn(async move {
								if let Err(e) = url_input_click(&input_value.read()).await {
									tracing::error!("Failed to handle url input click: {}", e);
								}
							});
						}
					},
				}
				button {
					onclick: move |_| {
						spawn(async move {
							if let Err(e) = url_input_click(&input_value.read()).await {
								tracing::error!("Failed to handle url input click: {}", e);
							}
						});
					},
					"Connect"
				}
			}
			div { class: "input-group",
				input { placeholder: "Enter the peer ID of the peer you want to connect to" }
				button { "Connect to Peer" }
			}
		}
	}
}

async fn url_input_click(url: &str) -> anyhow::Result<()> {
	let client_context = use_context::<ClientContext>();
	let mut client_context = client_context.lock().await;
	let client = wt::Client::new(url).await?;
	*client_context = Some(client);
	Ok(())
}
