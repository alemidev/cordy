use mlua::{UserData, Error};
use tokio::sync::broadcast;


#[derive(Clone)]
pub struct Console (broadcast::Sender<String>);

impl From::<broadcast::Sender<String>> for Console {
	fn from(channel: broadcast::Sender<String>) -> Self {
		Console(channel)
	}
}

impl UserData for Console {}
impl Console {
	pub fn send(&self, msg: String) -> Result<(), Error> {
		match self.0.send(msg) {
			Ok(_n_recv) => Ok(()),
			Err(e) => Err(Error::RuntimeError(format!("could not write to console: {}", e))),
		}
	}
}
