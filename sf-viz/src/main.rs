mod ui;
mod wt;

use std::sync::Arc;

use tokio::sync::Mutex;
use ui::*;

use dioxus::prelude::*;
use wt::Client;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
    #[route("/peers")]
    Peers {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

fn main() {
	dioxus_logger::init(tracing::Level::INFO).expect("failed to init logger");

	dioxus::launch(App);
}

#[component]
fn App() -> Element {
	wt::use_client_context_provider();
	rsx! {
		document::Link { rel: "icon", href: FAVICON }
		document::Link { rel: "stylesheet", href: MAIN_CSS }
		document::Link { rel: "stylesheet", href: TAILWIND_CSS }
		div { class: "app-wrapper", Router::<Route> {} }
	}
}
