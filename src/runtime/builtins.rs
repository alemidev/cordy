use std::{ffi::c_void, num::NonZeroUsize};

use mlua::{Lua, Error, Variadic, Value, ToLua};
use pox::{proc_maps::get_process_maps, tricks::fmt_path};
use nix::sys::{mman::{mprotect, ProtFlags, mmap, MapFlags, munmap}, signal::{Signal::SIGSEGV, SigHandler}};

use crate::helpers::pretty_lua;

use super::{console::Console, HELPTEXT};

const SIGSEGV_HOOK : AtomicBool = AtomicBool::new(false);

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

pub fn lua_hex(l: &Lua, (value, prefix): (Value, Option<bool>)) -> Result<String, Error> {
	let pre = if prefix.unwrap_or(true) { "0x" } else { "" };
	match value {
		Value::Nil        => Ok(format!("{}00", pre)),
		Value::Boolean(b) => Ok(format!("{}{:02X}", pre, b as i32)),
		Value::Integer(n) => Ok(format!("{}{:02X}", pre, n)),
		Value::String(s)  => Ok(
			s.as_bytes()
				.iter()
				.map(|x| format!("{:02X}", x))
				.fold(pre.into(), |acc, x| acc + x.as_str())
		),
		Value::Table(t)   => Ok(
			t.sequence_values::<Value>().into_iter()
				.filter_map(|x| if let Ok(v) = x { Some(v) } else { None })
				.map(|x| lua_hex(l, (x, Some(false))).unwrap_or("??".into())) // recursive! try stopping me
				.fold(pre.into(), |acc, x| acc + x.as_str())
		),
		Value::Number(_)        => Err(Error::RuntimeError("float has no hex value".into())),
		Value::Function(_)      => Err(Error::RuntimeError("function has no hex value".into())),
		Value::Thread(_)        => Err(Error::RuntimeError("thread has no hex value".into())),
		Value::LightUserData(_) => Err(Error::RuntimeError("LightUserData has no hex value".into())),
		Value::UserData(_)      => Err(Error::RuntimeError("UserData has no hex value".into())),
		Value::Error(_)         => Err(Error::RuntimeError("Error has no hex value".into())),
	}
}

/// could just use .to_ne_bytes() but lot of trailing zeros
fn i64_to_significant_bytes(n: i64) -> Vec<u8> {
	let mut out = vec![];
	for i in 0..8 {
		let val = (n >> (i*8)) as u8;
		let res = n >> ((i+1) * 8);
		if val == 0 && res == 0 { break; }
		out.push(val);
	}
	out
}

pub fn lua_bytes(l: &Lua, value: Value) -> Result<Vec<u8>, Error> {
	match value {
		Value::Nil => Ok(vec![]),
		Value::Boolean(b) => Ok(if b { vec![1] } else { vec![0] }),
		Value::Integer(n) => Ok(i64_to_significant_bytes(n)),
		Value::String(s) => Ok(s.as_bytes().to_vec()),
		Value::Table(t) => Ok(
			t.sequence_values::<Value>().into_iter()
				.filter_map(|x| if let Ok(v) = x { Some(v) } else { None })
				.map(|x| lua_bytes(l, x).unwrap_or(vec![]))
				.fold(vec![], |mut acc, mut x| { acc.append(&mut x); acc })
		),
		Value::Number(_)        => Err(Error::RuntimeError("cannot display float bytes value".into())),
		Value::Function(_)      => Err(Error::RuntimeError("cannot display function bytes value".into())),
		Value::Thread(_)        => Err(Error::RuntimeError("cannot display thread bytes value".into())),
		Value::LightUserData(_) => Err(Error::RuntimeError("cannot display LightUserData bytes value".into())),
		Value::UserData(_)      => Err(Error::RuntimeError("cannot display UserData bytes value".into())),
		Value::Error(_)         => Err(Error::RuntimeError("cannot display Error bytes value".into())),
	}
}

pub fn lua_read(_: &Lua, (addr, size): (usize, usize)) -> Result<Vec<u8>, Error> {
	if size == 0 {
		return Ok("".into());
	}
	let ptr = addr as *const u8;
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

extern fn handle_sigsegv(_signal: c_int) {
	eprintln!("Segmentation fault (ignored)");
}

pub fn lua_catch_sigsev(_: &Lua, mode: Option<bool>) -> Result<bool, Error> {
	match mode {
		Some(m) => match m {
			true => {
				let handler = SigHandler::Handler(handle_sigsegv);
				match unsafe { nix::sys::signal::signal(SIGSEGV, handler) } {
					Ok(_h) => {
						SIGSEGV_HOOK.store(true, Ordering::Relaxed);
						Ok(true)
					},
					Err(e) => Err(Error::RuntimeError(format!("could not set sig handler ({}): {}", e, e.desc()))),
				}
			},
			false => {
				match unsafe { nix::sys::signal::signal(SIGSEGV, SigHandler::SigDfl) } {
					Ok(_h) => {
						SIGSEGV_HOOK.store(false, Ordering::Relaxed);
						Ok(false)
					},
					Err(e) => Err(Error::RuntimeError(format!("could not reset sig handler ({}): {}", e, e.desc()))),
				}
			},
		},
		None => Ok(SIGSEGV_HOOK.load(Ordering::Relaxed)),
	}
}

pub fn lua_exit(_: &Lua, code: Option<i32>) -> Result<(), Error> {
	#[allow(unreachable_code)]
	Ok(std::process::exit(code.unwrap_or(0)))
}
