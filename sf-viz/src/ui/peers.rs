use dioxus::prelude::*;

/// Peers page
#[component]
pub fn Peers() -> Element {
	rsx! {
		div { id: "peers",
			h1 { "Peers" }
		}
	}
}
