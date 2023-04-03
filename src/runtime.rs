use std::{ffi::c_void, num::NonZeroUsize};

use mlua::{Lua, Variadic, Value, Error, UserData, ToLua};
use nix::sys::mman::{mmap, mprotect, ProtFlags, MapFlags, munmap};
use pox::{proc_maps::get_process_maps, tricks::fmt_path};
use pretty_hex::pretty_hex;
use tokio::sync::broadcast;

use crate::helpers::pretty_lua;

#[derive(Clone)]
struct Console (broadcast::Sender<String>);
impl UserData for Console {}

pub fn register_builtin_fn(lua: &Lua, console: broadcast::Sender<String>) -> Result<(), Error> {
	lua.globals().set("console", Console(console))?; // TODO passing it this way makes clones

	lua.globals().set("PROT_NONE",  ProtFlags::PROT_NONE.bits())?;
	lua.globals().set("PROT_READ",  ProtFlags::PROT_READ.bits())?;
	lua.globals().set("PROT_WRITE", ProtFlags::PROT_WRITE.bits())?;
	lua.globals().set("PROT_EXEC",  ProtFlags::PROT_EXEC.bits())?;

	lua.globals().set("MAP_ANON",   MapFlags::MAP_ANON.bits())?;
	lua.globals().set("MAP_PRIVATE",MapFlags::MAP_PRIVATE.bits())?;

	lua.globals().set("log",      lua.create_function(lua_log)?)?;
	lua.globals().set("hexdump",  lua.create_function(lua_hexdump)?)?;
	lua.globals().set("read",     lua.create_function(lua_read)?)?;
	lua.globals().set("write",    lua.create_function(lua_write)?)?;
	lua.globals().set("procmaps", lua.create_function(lua_procmaps)?)?;
	lua.globals().set("exit",     lua.create_function(lua_exit)?)?;
	lua.globals().set("mmap",     lua.create_function(lua_mmap)?)?;
	lua.globals().set("munmap",   lua.create_function(lua_munmap)?)?;
	lua.globals().set("mprotect", lua.create_function(lua_mprotect)?)?;
	lua.globals().set("help",     lua.create_function(lua_help)?)?;
	lua.globals().set("x",        lua.create_function(lua_hex)?)?;

	Ok(())
}

fn lua_help(lua: &Lua, _args: ()) -> Result<(), Error> {
	let console : Console = lua.globals().get("console")?;
	console.0.send(" > log([arg...])                    print to console rather than stdout\n".into()).unwrap();
	console.0.send(" > hexdump(bytes, [ret])            print hexdump of given bytes to console\n".into()).unwrap();
	console.0.send(" > exit([code])                     immediately terminate process\n".into()).unwrap();
	console.0.send(" > mmap([a], l, [p], [f], [d], [o]) execute mmap syscall\n".into()).unwrap();
	console.0.send(" > munmap(addr, len)                unmap {size} bytes at {addr}\n".into()).unwrap();
	console.0.send(" > mprotect(addr, len, prot)        set permission flags on target memory area\n".into()).unwrap();
	console.0.send(" > procmaps([ret])                  returns process memory maps as string\n".into()).unwrap();
	console.0.send(" > read(addr, size)                 read raw bytes at given address\n".into()).unwrap();
	console.0.send(" > write(addr, bytes)               write raw bytes at given address\n".into()).unwrap();
	console.0.send(" > x(n)                             show hex representation of given number\n".into()).unwrap();
	console.0.send(" > help()                           print these messages".into()).unwrap();
	Ok(())
}

fn lua_log(lua: &Lua, values: Variadic<Value>) -> Result<usize, Error> {
	let mut out = String::new();
	let console : Console = lua.globals().get("console")?;
	for value in values {
		out.push_str(&pretty_lua(value));
		out.push(' ');
	}
	out.push('\n');
	let size = out.len();
	console.0.send(out).unwrap();
	Ok(size)
}

fn lua_hexdump(lua: &Lua, (bytes, ret): (Vec<u8>, Option<bool>)) -> Result<Value, Error> {
	let txt = pretty_hex(&bytes) + "\n";
	if ret.is_some() && ret.unwrap() {
		return Ok(txt.to_lua(lua)?);
	}
	let console : Console = lua.globals().get("console")?;
	match console.0.send(txt) {
		Ok(n) => Ok(n.to_lua(lua)?),
		Err(e) => Err(Error::RuntimeError(format!("could not convert bytes to hexdump: {}", e))),
	}
}

fn lua_hex(_: &Lua, n: usize) -> Result<String, Error> {
	Ok(format!("0x{:X}", n))
}

fn lua_read(_: &Lua, (addr, size): (usize, usize)) -> Result<Vec<u8>, Error> {
	if size == 0 {
		return Ok("".into());
	}
	let ptr = addr as *mut u8;
	let slice = unsafe { std::slice::from_raw_parts(ptr, size) };
	Ok(slice.to_vec())
}

fn lua_write(_: &Lua, (addr, data): (usize, Vec<u8>)) -> Result<usize, Error> {
	for (i, byte) in data.iter().enumerate() {
		let off = (addr + i) as *mut u8;
		unsafe { *off = *byte } ;
	}
	Ok(data.len())
}

fn lua_procmaps(lua: &Lua, ret: Option<bool>) -> Result<Value, Error> {
	let mut out = String::new();
	for map in get_process_maps(std::process::id() as i32).unwrap() {
		out.push_str(
			format!(
				"[{}] 0x{:08X}..0x{:08X} +{:08x} ({}b) \t {} {}\n",
				map.flags, map.start(), map.start() + map.size(), map.offset, map.size(), fmt_path(map.filename()),
				if map.inode != 0 { format!("({})", map.inode) } else { "".into() },
			).as_str()
		);
	}
	if ret.is_some() && ret.unwrap() {
		return Ok(out.to_lua(lua)?);
	}
	let console : Console = lua.globals().get("console")?;
	let written = print(console, out)?;
	Ok(written.to_lua(lua)?)
}

fn lua_mprotect(_: &Lua, (addr, size, prot): (usize, usize, i32)) -> Result<(), Error> {
	match unsafe { mprotect(addr as *mut c_void, size, ProtFlags::from_bits_truncate(prot)) } {
		Ok(()) => Ok(()),
		Err(e) => Err(Error::RuntimeError(format!("could not run mprotect ({}): {}", e, e.desc()))),
	}
}

fn lua_mmap(_: &Lua, (addr, length, prot, flags, fd, offset): (Option<usize>, usize, Option<i32>, Option<i32>, Option<i32>, Option<i64>)) -> Result<usize, Error> {
	if length <= 0 {
		return Ok(0); // TODO make this an Err
	}
	match unsafe { mmap(
		if let Some(a) = addr { NonZeroUsize::new(a) } else { None },
		NonZeroUsize::new(length).unwrap(),
		if let Some(p) = prot { ProtFlags::from_bits_truncate(p) } else { ProtFlags::PROT_READ | ProtFlags::PROT_WRITE },
		if let Some(f) = flags { MapFlags::from_bits_truncate(f) } else { MapFlags::MAP_PRIVATE | MapFlags::MAP_ANON },
		fd.unwrap_or(-1),
		offset.unwrap_or(0),
	) } {
		Ok(x) => Ok(x as usize),
		Err(e) => Err(Error::RuntimeError(format!("could not run mmap ({}): {}", e, e.desc()))),
	}
}

fn lua_munmap(_: &Lua, (addr, len): (usize, usize)) -> Result<(), Error> {
	match unsafe { munmap(addr as *mut c_void, len) } {
		Ok(()) => Ok(()),
		Err(e) => Err(Error::RuntimeError(format!("could not run munmap ({}): {}", e, e.desc()))),
	}
}

fn lua_exit(_: &Lua, code: Option<i32>) -> Result<(), Error> {
	#[allow(unreachable_code)]
	Ok(std::process::exit(code.unwrap_or(0)))
}

fn print(console: Console, txt: String) -> Result<usize, Error> {
	match console.0.send(txt) {
		Ok(n) => Ok(n),
		Err(e) => Err(Error::RuntimeError(format!("could not print message to console: {}", e))),
	}
}
