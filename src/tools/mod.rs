use mlua::{Lua, Error};
use nix::sys::mman::{ProtFlags, MapFlags};
use tokio::sync::broadcast;

use crate::console::Console;

use self::format::GLOBAL_CONSOLE;

pub mod format;
pub mod memory;
pub mod syscall;
pub mod proc;

pub mod dumb;

use self::dumb::*;
use self::format::*;
use self::memory::*;
use self::proc::*;
use self::syscall::*;

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
	lua.globals().set("decomp",   lua.create_function(lua_decomp)?)?;
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
