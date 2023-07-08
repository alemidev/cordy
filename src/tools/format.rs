use iced_x86::{Decoder, DecoderOptions, IntelFormatter, Instruction, Formatter};
use mlua::{Lua, Error, Variadic, Value, ToLua};

use crate::{helpers::pretty_lua, console::Console};

pub const GLOBAL_CONSOLE : &str = "GLOBAL_CONSOLE";

pub const HELPTEXT : &str = "?> This is a complete lua repl
?> Make scripts or just evaluate expressions
?> print() will go to original process stdout, use log()
?> to send to this console instead
?> Each connection will spawn a fresh repl, but only one
?> concurrent connection is allowed
?> Some ad-hoc functions to work with affected process
?> are already available in this repl globals:
 >  log([arg...])                    print to console rather than stdout
 >  hexdump(bytes, [ret])            print hexdump of given {bytes} to console
 >  exit([code])                     immediately terminate process
 >  mmap([a], l, [p], [f], [d], [o]) execute mmap syscall
 >  munmap(ptr, len)                 unmap {len} bytes at {ptr}
 >  mprotect(ptr, len, prot)         set {prot} flags from {ptr} to {ptr+len}
 >  procmaps([ret])                  get process memory maps as string
 >  threads([ret])                   get process threads list as string
 >  read(addr, size)                 read {size} raw bytes at {addr}
 >  write(addr, bytes)               write given {bytes} at {addr}
 >  find(ptr, len, match, [first])   search from {ptr} to {ptr+len} for {match} and return addrs
 >  x(number, [prefix])              show hex representation of given {number}
 >  b(string)                        return array of bytes from given {string}
 >  sigsegv([set])                   get or set SIGSEGV handler state
 >  help()                           print these messages
";


pub fn lua_help(lua: &Lua, _args: ()) -> Result<(), Error> {
	let console : Console = lua.globals().get(GLOBAL_CONSOLE)?;
	console.send(HELPTEXT.into())?;
	Ok(())
}

pub fn lua_log(lua: &Lua, values: Variadic<Value>) -> Result<usize, Error> {
	let mut out = String::new();
	let console : Console = lua.globals().get(GLOBAL_CONSOLE)?;
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
	if ret.unwrap_or(false) {
		return Ok(pretty_hex::simple_hex(&bytes).to_lua(lua)?);
	}
	let console : Console = lua.globals().get(GLOBAL_CONSOLE)?;
	console.send(pretty_hex::pretty_hex(&bytes) + "\n")?;
	Ok(Value::Nil)
}

fn padding(size: i32) -> String {
	if size <= 0 {
		"".into()
	} else {
		(0..size as usize).map(|_| " ").collect::<String>()
	}
}

pub fn lua_decomp(lua: &Lua, (bytes, ret): (Vec<u8>, Option<bool>)) -> Result<Value, Error> {
	let ret_value = ret.unwrap_or(false);
	let bitness = 8 * std::mem::size_of::<usize>() as u32;
	let mut decoder = Decoder::with_ip(bitness, bytes.as_slice(), 0, DecoderOptions::NONE);
	let mut formatter = IntelFormatter::new();
	let mut instr_buffer = String::new();
	let mut raw_buffer = String::new();
	let mut instruction = Instruction::default();
	let mut output = String::new();
	let mut retval = vec![];
	let mut count = 0;
	while decoder.can_decode() {
		decoder.decode_out(&mut instruction);
		instr_buffer.clear();
		formatter.format(&instruction, &mut instr_buffer);
		if ret_value {
			retval.push(instr_buffer.clone());
			continue;
		}
		raw_buffer.clear();
		let start_index = instruction.ip() as usize;
		let instrs_bytes = &bytes[start_index..start_index+instruction.len()];
		for b in instrs_bytes {
			raw_buffer.push_str(&format!("{:02x} ", b));
		}
		let padding = padding(30 - raw_buffer.len() as i32);
		output.push_str(&format!("{:08X}:      {}{}{}\n", instruction.ip(), raw_buffer, padding, instr_buffer));
		count += 1;
	}
	if ret_value {
		Ok(retval.to_lua(lua)?)
	} else {
		let console : Console = lua.globals().get(GLOBAL_CONSOLE)?;
		console.send(output)?;
		Ok(count.to_lua(lua)?)
	}
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
