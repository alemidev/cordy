use mlua::{Lua, MultiValue, Variadic, Value, Table};
use tokio::{sync::{mpsc, broadcast}, net::{TcpStream, TcpListener}, io::{AsyncWriteExt, AsyncReadExt}};
use tracing::{error, debug};

use pox::proc_maps::get_process_maps;
use pox::tricks::fmt_path;

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
		let mut lua = Lua::new();
		prepare_lua_runtime(&mut lua, self.source.clone());
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

fn prepare_lua_runtime(lua: &Lua, console: broadcast::Sender<String>) {
	let c = console.clone();
	let log = lua.create_function(move |_lua, values: Variadic<Value>| {
		let mut out = String::new();
		for value in values {
			out.push_str(&pretty_lua(value));
			out.push(' ');
		}
		out.push('\n');
		let size = out.len();
		c.send(out).unwrap();
		Ok(size)
	}).unwrap();
	lua.globals().set("log", log).unwrap();

	let procmaps = lua.create_function(move |_lua, ()| {
		let mut out = String::new();
		for map in get_process_maps(std::process::id() as i32).unwrap() {
			out.push_str(
				format!(
					"[{}] 0x{:08X}..0x{:08X} +{:08x} \t {} {}\n",
					map.flags, map.start(), map.start() + map.size(), map.offset, fmt_path(map.filename()),
					if map.inode != 0 { format!("({})", map.inode) } else { "".into() },
				).as_str()
			);
		}
		Ok(out)
	}).unwrap();
	lua.globals().set("procmaps", procmaps).unwrap();

	let hexdump = lua.create_function(move |_lua, (addr, size): (usize, usize)| {
		if size == 0 {
			return Ok("".into());
		}
		let ptr = addr as *mut u8;
		let slice = unsafe { std::slice::from_raw_parts(ptr, size) };
		let mut out = String::new();
		for line in hexdump::hexdump_iter(slice) {
			out.push_str(&line);
			out.push('\n');
		}
		Ok(out)
	}).unwrap();
	lua.globals().set("hexdump", hexdump).unwrap();

	let write = lua.create_function(move |_lua, (addr, data): (usize, Vec<u8>)| {
		for (i, byte) in data.iter().enumerate() {
			let off = (addr + i) as *mut u8;
			unsafe { *off = *byte } ;
		}
		Ok(data.len())
	}).unwrap();
	lua.globals().set("write", write).unwrap();

	let exit = lua.create_function(move |_lua, code: i32| {
		#[allow(unreachable_code)]
		Ok(std::process::exit(code))
	}).unwrap();
	lua.globals().set("exit", exit).unwrap();

	let help = lua.create_function(move |_lua, ()| {
		console.send(" > log(...)              print to (this) remote shell\n".into()).unwrap();
		console.send(" > exit(code)            immediately terminate process\n".into()).unwrap();
		console.send(" > procmaps()            returns process memory maps as string\n".into()).unwrap();
		console.send(" > write(addr, bytes)    write raw bytes at given address\n".into()).unwrap();
		console.send(" > hexdump(addr, size)   dump bytes at addr in hexdump format\n".into()).unwrap();
		console.send(" > help()                print these messages".into()).unwrap();
		Ok(())
	}).unwrap();
	lua.globals().set("help", help).unwrap();
}

fn pretty_lua(val: Value) -> String {
	// TODO there must be some builtin to do this, right???
	match val {
		Value::Nil => "nil".into(),
		Value::Boolean(b) => if b { "true".into() } else { "false".into() },
		Value::LightUserData(x) => format!("LightUserData({:?})", x),
		Value::Integer(n) => format!("{}", n),
		Value::Number(n) => format!("{:.3}", n),
		Value::String(s) => s.to_str().expect("string is not str").into(),
		Value::Table(t) => try_serialize_table(&t),
		Value::Function(f) => format!("Function({:?}", f),
		Value::Thread(t) => format!("Thread({:?})", t),
		Value::UserData(x) => format!("UserData({:?})", x),
		Value::Error(e) => format!("Error({:?}) : {}", e, e.to_string()),
	}
}

fn try_serialize_table(t: &Table) -> String {
	match serde_json::to_string(t) {
		Ok(txt) => txt,
		Err(_e)  => format!("{:?}", t),
	}
}
