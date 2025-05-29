use std::{future::Future, pin::Pin};

use futures::executor::ThreadPool;

pub trait Executor {
	/// Run the given future in the background until it ends.
	#[track_caller]
	fn exec(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>);
}

impl<F: Fn(Pin<Box<dyn Future<Output = ()> + Send>>)> Executor for F {
	fn exec(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
		self(f)
	}
}

impl Executor for ThreadPool {
	fn exec(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>) {
		self.spawn_ok(future)
	}
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct TokioExecutor;
#[cfg(not(target_arch = "wasm32"))]
impl Executor for TokioExecutor {
	fn exec(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>) {
		tokio::spawn(future);
	}
}

#[cfg(target_arch = "wasm32")]
#[derive(Default, Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct WasmBindgenExecutor;
#[cfg(target_arch = "wasm32")]
impl Executor for WasmBindgenExecutor {
	fn exec(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>) {
		wasm_bindgen_futures::spawn_local(future)
	}
}

pub(crate) fn get_executor() -> impl Executor {
	#[cfg(not(target_arch = "wasm32"))]
	{
		TokioExecutor
	}
	#[cfg(target_arch = "wasm32")]
	{
		WasmBindgenExecutor
	}
}
