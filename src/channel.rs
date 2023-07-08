use mlua::Lua;
use tokio::{sync::{mpsc, broadcast}, net::{TcpListener, TcpStream}, io::{AsyncWriteExt, AsyncReadExt}};
use tracing::{debug, error, warn};

use crate::{repl::{LuaRepl, VERSIONTEXT}, tools::register_builtin_fn};

pub struct ControlChannel {
	addr: String,
	sink: mpsc::Receiver<String>,
	source: broadcast::Sender<String>,
}

pub struct ControlChannelHandle {
	pub sink: mpsc::Sender<String>,
	pub source: broadcast::Receiver<String>,
}

impl ControlChannel {
	pub fn run(addr: String) -> ControlChannelHandle {
		let (sink_tx, sink_rx) = mpsc::channel(64);
		let (source_tx, source_rx) = broadcast::channel(64);
		let mut chan = ControlChannel {
			addr,
			sink: sink_rx,
			source: source_tx,
		};

		tokio::spawn(async move { chan.work().await });

		ControlChannelHandle { sink: sink_tx, source: source_rx }
	}

	async fn work(&mut self) {
		match TcpListener::bind(&self.addr).await {
			Ok(listener) => {
				loop {
					match listener.accept().await {
						Ok((stream, addr)) => {
							debug!("accepted connection from {}, serving shell", addr);
							self.process(stream).await;
						}
						Err(e) => error!("could not accept connection: {}", e),
					}
				}
			},
			Err(e) => error!("could not bind on {} : {}", self.addr, e),
		}
	}

	async fn process(&mut self, mut stream: TcpStream) {
		let mut lua = Lua::new();
		let mut repl = LuaRepl::new(self.source.clone().into());

		let intro_text = format!(
			"{} inside process #{}\n@> ",
			VERSIONTEXT, std::process::id()
		);

		if let Err(e) = repl.write(intro_text) {
			warn!("could not display version on repl: {}", e);
		}

		if let Err(e) = register_builtin_fn(&mut lua, self.source.clone()) {
			error!("could not prepare runtime environment: {}", e);
		}

		loop {
			tokio::select! {

				rx = stream.read_u8() => match rx { // TODO should be cancelable, but is it really?
					Ok(c) => {
						if !c.is_ascii() {
							debug!("character '{}' is not ascii", c);
							break;
						}
						let ch : char = c as char;
						if let Err(e) = repl.evaluate(&lua, ch) {
							error!("could not evaluate input '{}' : {}", repl.buffer(), e);
						}
					},
					Err(e) => {
						debug!("lost connection: {}", e);
						break;
					}
				},

				tx = self.sink.recv() => match tx {
					Some(txt) => {
						if let Err(e) = stream.write_all(txt.as_bytes()).await {
							error!("could not send output to remote console: {}", e);
							break;
						}
					}
					None => {
						error!("command sink closed, exiting processor");
						break;
					}
				},

			}
		}
	}
}
