use mlua::{Lua, MultiValue};
use tokio::{sync::{mpsc, broadcast}, net::{TcpStream, TcpListener}, io::{AsyncWriteExt, AsyncReadExt}};
use tracing::{error, debug, info};

#[ctor::ctor]
fn contructor() {
	std::thread::spawn(move || {
		tracing_subscriber::fmt()
			.with_max_level(tracing::Level::DEBUG)
			.with_writer(std::io::stderr)
			.init();
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

struct ControlChannel {
	addr: String,
	sink: mpsc::Receiver<String>,
	source: broadcast::Sender<String>,
}

pub struct ControlChannelHandle {
	sink: mpsc::Sender<String>,
	source: broadcast::Receiver<String>,
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
		let lua = Lua::new();
		self.source.send(
			format!("LuaJit 5.2 via rlua inside process #{}\n@> ", std::process::id())
		).unwrap();
		let mut cmd = String::new();
		loop {
			tokio::select! {

				rx = stream.read_u8() => match rx { // FIXME is read_exact cancelable?
					Ok(c) => {
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
											let x = serde_json::to_string(&val).unwrap();
											self.source.send(format!("@({}) : {}",val.type_name(), x)).unwrap();
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
