pub mod builtins;
pub mod console;
pub mod repl;

use mlua::{Lua, Error};
use nix::sys::mman::{ProtFlags, MapFlags};

use tokio::sync::broadcast;

use crate::runtime::console::Console;
use crate::runtime::builtins::*;

pub const GLOBAL_CONSOLE : &str = "GLOBAL_CONSOLE";

pub fn register_builtin_fn(lua: &Lua, console: broadcast::Sender<String>) -> Result<(), Error> {
	lua.globals().set(GLOBAL_CONSOLE, Console::from(console))?; // TODO passing it this way makes clones

	lua.globals().set("PROT_NONE",  ProtFlags::PROT_NONE.bits())?;
	lua.globals().set("PROT_READ",  ProtFlags::PROT_READ.bits())?;
	lua.globals().set("PROT_WRITE", ProtFlags::PROT_WRITE.bits())?;
	lua.globals().set("PROT_EXEC",  ProtFlags::PROT_EXEC.bits())?;
	lua.globals().set("PROT_ALL",  (ProtFlags::PROT_EXEC | ProtFlags::PROT_WRITE | ProtFlags::PROT_READ).bits())?;

	lua.globals().set("MAP_ANON",   MapFlags::MAP_ANON.bits())?;
	lua.globals().set("MAP_PRIVATE",MapFlags::MAP_PRIVATE.bits())?;

	lua.globals().set("log",      lua.create_function(lua_log)?)?;
	lua.globals().set("hexdump",  lua.create_function(lua_hexdump)?)?;
	lua.globals().set("read",     lua.create_function(lua_read)?)?;
	lua.globals().set("write",    lua.create_function(lua_write)?)?;
	lua.globals().set("find",     lua.create_function(lua_find)?)?;
	lua.globals().set("procmaps", lua.create_function(lua_procmaps)?)?;
	lua.globals().set("threads",  lua.create_function(lua_threads)?)?;
	lua.globals().set("exit",     lua.create_function(lua_exit)?)?;
	lua.globals().set("mmap",     lua.create_function(lua_mmap)?)?;
	lua.globals().set("munmap",   lua.create_function(lua_munmap)?)?;
	lua.globals().set("mprotect", lua.create_function(lua_mprotect)?)?;
	lua.globals().set("sigsegv",  lua.create_function(lua_catch_sigsev)?)?;
	lua.globals().set("help",     lua.create_function(lua_help)?)?;
	lua.globals().set("x",        lua.create_function(lua_hex)?)?;
	lua.globals().set("b",        lua.create_function(lua_bytes)?)?;

	Ok(())
}

pub const VERSIONTEXT : &str = "LuaJit 5.2 via rlua";
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
