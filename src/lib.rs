mod channel;
mod helpers;
mod console;
mod repl;
mod tools;

use channel::ControlChannel;
use tracing::error;

#[ctor::ctor]
fn contructor() {
	std::thread::spawn(move || -> Result<(), std::io::Error> {
		tracing_subscriber::fmt()
			.with_max_level(tracing::Level::DEBUG)
			.with_writer(std::io::stderr)
			.init();
		tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()?
			.block_on(main());
		Ok(())
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
