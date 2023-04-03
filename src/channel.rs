use mlua::{Lua, MultiValue};
use tokio::{sync::{mpsc, broadcast}, net::{TcpListener, TcpStream}, io::{AsyncWriteExt, AsyncReadExt}};
use tracing::{debug, error};

use crate::{runtime::register_builtin_fn, helpers::pretty_lua};


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
		if let Err(e) = register_builtin_fn(&mut lua, self.source.clone()) {
			error!("could not prepare Lua runtime: {}", e);
			return;
		}

		self.source.send(
			format!("LuaJit 5.2 via rlua inside process #{}\n@> ", std::process::id())
		).unwrap();
		let mut cmd = String::new();
		loop {
			tokio::select! {

				rx = stream.read_u8() => match rx { // FIXME is read_exact cancelable?
					Ok(c) => {
						// TODO move this "lua repl" code outside of here!
						if !c.is_ascii() {
							debug!("character '{}' is not ascii", c);
							break;
						}
						let ch : char = c as char;
						match ch {
							'\u{8}' => {
								if cmd.len() > 0 {
									cmd.remove(cmd.len() - 1);
								}
							},
							'\n' => {
								match lua.load(&cmd).eval::<MultiValue>() {
									Ok(values) => {
										for val in values {
											self.source.send(format!("=({}) {}", val.type_name(), pretty_lua(val))).unwrap();
										}
										self.source.send("\n@> ".into()).unwrap();
										cmd = String::new();
									},
									Err(e) => {
										match e {
											mlua::Error::SyntaxError { message: _, incomplete_input: true } => {
												self.source.send("@    ".into()).unwrap();
												cmd.push(ch);
											},
											_ => {
												self.source.send(format!("! {}\n@> ", e)).unwrap();
												cmd = String::new();
											},
										}
									}
								}
							},
							'\0' => break,
							_ => cmd.push(ch),
						}
					},
					Err(e) => {
						debug!("lost connection: {}", e);
						break;
					}
				},

				tx = self.sink.recv() => match tx {
					Some(txt) => stream.write_all(txt.as_bytes()).await.unwrap(),
					None => {
						error!("command sink closed, exiting processor");
						break;
					}
				},
			}
		}
	}
}
