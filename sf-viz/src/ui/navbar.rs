use dioxus::prelude::*;

use crate::Route;

/// Shared navbar component.
#[component]
pub fn Navbar() -> Element {
	rsx! {
		div { id: "navbar",
			div { class: "nav-links",
				Link { to: Route::Home {}, "Home" }
				Link { to: Route::Peers {}, "Peer list" }
			}
		}
		Outlet::<Route> {}
	}
}
