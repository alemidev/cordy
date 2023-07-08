use mlua::{Lua, Error};

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
