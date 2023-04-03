mod runtime;
mod channel;

use channel::ControlChannel;
use tracing::{error, debug};

#[ctor::ctor]
fn contructor() {
	eprint!(" -[infected]- ");
	std::thread::spawn(move || {
		tracing_subscriber::fmt()
			.with_max_level(tracing::Level::DEBUG)
			.with_writer(std::io::stderr)
			.init();
		debug!("infected process");
		tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.unwrap()
			.block_on(main());
	});
}

#[ctor::dtor]
fn destructor() {}

async fn main() {
	let mut handle = ControlChannel::run("127.0.0.1:13337".into());

	loop {
		match handle.source.recv().await {
			Ok(txt) => {
				if let Err(e) = handle.sink.send(txt).await {
					error!("could not echo back txt: {}", e);
				}
			},
			Err(e) => {
				error!("controller data channel is closed: {}", e);
				break;
			}
		}
	}
}

