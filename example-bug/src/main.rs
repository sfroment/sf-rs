use futures::future::BoxFuture;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::pin::Pin;

pub trait SomeInnerTrait: Send + Sync + 'static {}
pub trait SomeOtherTrait: Error + Send + Sync + 'static {}

pub trait SomeTrait: Send + Sync + 'static {
	type Output;
	type Error;

	//type Dial: Future<Output = Result<Self::Output, Self::Error>> + Send + 'static;
	fn dial(
		&self,
		addr: String,
	) -> Result<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>, Self::Error>;
}

#[derive(Debug)]
pub struct BoxedError(Box<dyn std::error::Error + Send + Sync + 'static>);

impl fmt::Display for BoxedError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl Error for BoxedError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		self.0.source()
	}
}

type BoxedOutput = Box<dyn SomeInnerTrait>;

pub struct Test {
	transports: HashMap<
		String,
		Box<
			dyn SomeTrait<Output = Box<dyn SomeInnerTrait>, Error = Box<dyn std::error::Error + Send + Sync + 'static>>,
		>,
	>,
}

impl Test {
	pub fn call(&self, addr: String) -> Result<(), std::io::Error> {
		let dial = self
			.transports
			.get(&addr)
			.ok_or_else(|| std::io::Error::other("some i/o error"))?;
		dial.dial(addr).map_err(|e| std::io::Error::other("test"))?;
		Ok(())
	}
}

fn main() {}
