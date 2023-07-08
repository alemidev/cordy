use std::{ffi::c_void, num::NonZeroUsize};

use mlua::{Lua, Error};
use nix::sys::mman::{mprotect, ProtFlags, mmap, MapFlags, munmap};

use cordy_macro::lua_fn;

#[lua_fn]
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
