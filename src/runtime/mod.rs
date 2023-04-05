pub mod builtins;
pub mod console;
pub mod repl;

use mlua::{Lua, Error};
use nix::sys::mman::{ProtFlags, MapFlags};

use tokio::sync::broadcast;

use crate::runtime::console::Console;
use crate::runtime::builtins::*;


pub fn register_builtin_fn(lua: &Lua, console: broadcast::Sender<String>) -> Result<(), Error> {
	lua.globals().set("console", Console::from(console))?; // TODO passing it this way makes clones

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

pub const VERSIONTEXT : &str = "LuaJit 5.2 via rlua";
pub const HELPTEXT : &str = "?> this is a complete lua repl
?> you can make scripts or just evaluate expressions
?> print() will go to original process stdout, use log()
?> to send to this console instead
?> each connection will spawn a fresh repl, but only one
?> concurrent connection is allowed
?> some ad-hoc functions to work with affected process
?> are already available in this repl globals:
 >  log([arg...])                    print to console rather than stdout
 >  hexdump(bytes, [ret])            print hexdump of given bytes to console
 >  exit([code])                     immediately terminate process
 >  mmap([a], l, [p], [f], [d], [o]) execute mmap syscall
 >  munmap(addr, len)                unmap {size} bytes at {addr}
 >  mprotect(addr, len, prot)        set permission flags on target memory area
 >  procmaps([ret])                  returns process memory maps as string
 >  read(addr, size)                 read raw bytes at given address
 >  write(addr, bytes)               write raw bytes at given address
 >  x(n)                             show hex representation of given number
 >  help()                           print these messages
";
