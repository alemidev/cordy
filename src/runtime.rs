use mlua::{Lua, Variadic, Value, Table};
use pox::{proc_maps::get_process_maps, tricks::fmt_path};
use tokio::sync::broadcast;


pub fn prepare_lua_runtime(lua: &Lua, console: broadcast::Sender<String>) {
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

	let exit = lua.create_function(move |_lua, code: Option<i32>| {
		#[allow(unreachable_code)]
		Ok(std::process::exit(code.unwrap_or(0)))
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

pub fn pretty_lua(val: Value) -> String {
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
