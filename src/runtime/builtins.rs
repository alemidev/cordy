use std::{ffi::c_void, num::NonZeroUsize};

use mlua::{Lua, Error, Variadic, Value, ToLua};
use nix::sys::mman::{mprotect, ProtFlags, mmap, MapFlags, munmap};
use pox::{proc_maps::get_process_maps, tricks::fmt_path};

use crate::helpers::pretty_lua;

use super::{console::Console, HELPTEXT};

pub fn lua_help(lua: &Lua, _args: ()) -> Result<(), Error> {
	let console : Console = lua.globals().get("console")?;
	console.send(HELPTEXT.into())
}

pub fn lua_log(lua: &Lua, values: Variadic<Value>) -> Result<usize, Error> {
	let mut out = String::new();
	let console : Console = lua.globals().get("console")?;
	for value in values {
		out.push_str(&pretty_lua(value));
		out.push(' ');
	}
	out.push('\n');
	let size = out.len();
	console.send(out)?;
	Ok(size)
}

pub fn lua_hexdump(lua: &Lua, (bytes, ret): (Vec<u8>, Option<bool>)) -> Result<Value, Error> {
	let txt = pretty_hex::pretty_hex(&bytes) + "\n";
	if ret.unwrap_or(false) {
		return Ok(txt.to_lua(lua)?);
	}
	let console : Console = lua.globals().get("console")?;
	console.send(txt)?;
	Ok(Value::Nil)
}

pub fn lua_hex(_: &Lua, n: usize) -> Result<String, Error> {
	Ok(format!("0x{:X}", n))
}

pub fn lua_read(_: &Lua, (addr, size): (usize, usize)) -> Result<Vec<u8>, Error> {
	if size == 0 {
		return Ok("".into());
	}
	let ptr = addr as *mut u8;
	let slice = unsafe { std::slice::from_raw_parts(ptr, size) };
	Ok(slice.to_vec())
}

pub fn lua_write(_: &Lua, (addr, data): (usize, Vec<u8>)) -> Result<usize, Error> {
	for (i, byte) in data.iter().enumerate() {
		let off = (addr + i) as *mut u8;
		unsafe { *off = *byte } ;
	}
	Ok(data.len())
}

pub fn lua_find(
	_: &Lua, (start, size, pattern, first): (usize, usize, Vec<u8>, Option<bool>)
) -> Result<Vec<usize>, Error> {
	let window = pattern.len();
	let first_only = first.unwrap_or(false);
	let mut matches = vec![];

	for i in 0..(size-window) {
		let slice = unsafe { std::slice::from_raw_parts((start + i) as *const u8, window) };
		if slice == pattern {
			matches.push(start + i);
			if first_only { break; }
		}
	}

	Ok(matches)
}

pub fn lua_procmaps(lua: &Lua, ret: Option<bool>) -> Result<Value, Error> {
	let mut out = String::new();
	let maps = get_process_maps(std::process::id() as i32)
		.map_err(|e| Error::RuntimeError(
			format!("could not obtain process maps: {}", e)
		))?;
	for map in maps {
		out.push_str(
			format!(
				"[{}] 0x{:08X}..0x{:08X} +{:08x} ({}b) \t {} {}\n",
				map.flags, map.start(), map.start() + map.size(), map.offset, map.size(), fmt_path(map.filename()),
				if map.inode != 0 { format!("({})", map.inode) } else { "".into() },
			).as_str()
		);
	}
	if ret.unwrap_or(false) {
		return Ok(out.to_lua(lua)?);
	}
	let console : Console = lua.globals().get("console")?;
	console.send(out)?;
	Ok(Value::Nil)
}

pub fn lua_mprotect(_: &Lua, (addr, size, prot): (usize, usize, i32)) -> Result<(), Error> {
	match unsafe { mprotect(addr as *mut c_void, size, ProtFlags::from_bits_truncate(prot)) } {
		Ok(()) => Ok(()),
		Err(e) => Err(Error::RuntimeError(format!("could not run mprotect ({}): {}", e, e.desc()))),
	}
}

pub fn lua_mmap(_: &Lua, (addr, length, prot, flags, fd, offset): (Option<usize>, usize, Option<i32>, Option<i32>, Option<i32>, Option<i64>)) -> Result<usize, Error> {
	if length <= 0 {
		return Ok(0); // TODO make this an Err
	}
	match unsafe { mmap(
		if let Some(a) = addr { NonZeroUsize::new(a) } else { None },
		NonZeroUsize::new(length).unwrap(), // safe because we manually checked lenght to be > 0
		if let Some(p) = prot { ProtFlags::from_bits_truncate(p) } else { ProtFlags::PROT_READ | ProtFlags::PROT_WRITE },
		if let Some(f) = flags { MapFlags::from_bits_truncate(f) } else { MapFlags::MAP_PRIVATE | MapFlags::MAP_ANON },
		fd.unwrap_or(-1),
		offset.unwrap_or(0),
	) } {
		Ok(x) => Ok(x as usize),
		Err(e) => Err(Error::RuntimeError(format!("could not run mmap ({}): {}", e, e.desc()))),
	}
}

pub fn lua_munmap(_: &Lua, (addr, len): (usize, usize)) -> Result<(), Error> {
	match unsafe { munmap(addr as *mut c_void, len) } {
		Ok(()) => Ok(()),
		Err(e) => Err(Error::RuntimeError(format!("could not run munmap ({}): {}", e, e.desc()))),
	}
}

pub fn lua_exit(_: &Lua, code: Option<i32>) -> Result<(), Error> {
	#[allow(unreachable_code)]
	Ok(std::process::exit(code.unwrap_or(0)))
}
