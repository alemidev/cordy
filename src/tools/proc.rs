use mlua::{Lua, Error, Table, Value, ToLua};
use procfs::{process::{Status, MemoryMap, Process, MemoryMaps, Task, TasksIter}, ProcResult, ProcError};
use tracing::warn;

use crate::console::Console;

use super::format::GLOBAL_CONSOLE;


fn proc_table(lua: &Lua, task: Status) -> Result<Table, Error> {
	let table = lua.create_table()?;
	table.set("pid", task.pid)?;
	table.set("name", task.name)?;
	table.set("state", task.state)?;
	table.set("fdsize", task.fdsize)?;
	Ok(table)
}

fn map_table(lua: &Lua, task: MemoryMap) -> Result<Table, Error> {
	let table = lua.create_table()?;
	table.set("perms", task.perms.as_str())?;
	table.set("address", task.address.0)?;
	table.set("offset", task.offset)?;
	table.set("size", task.address.1 - task.address.0)?;
	table.set("path", format!("{:?}", task.pathname))?;
	Ok(table)
}

fn proc_maps() -> ProcResult<MemoryMaps> {
	Ok(Process::myself()?.maps()?)
}

pub fn lua_procmaps(lua: &Lua, ret: Option<bool>) -> Result<Value, Error> {
	let maps = proc_maps()
		.map_err(|e| Error::RuntimeError(
			format!("could not obtain process maps: {}", e)
		))?;
	if ret.unwrap_or(false) {
		let mut out = vec![];
		for map in maps {
			out.push(map_table(lua, map)?);
		}
		Ok(out.to_lua(lua)?)
	} else {
		let mut out = String::new();
		let mut count = 0;
		for map in maps {
			count += 1;
			out.push_str(
				format!(
					" * [{}] 0x{:08X}..0x{:08X} +{:08x} ({}b) \t {:?} {}\n",
					map.perms.as_str(), map.address.0, map.address.1, map.offset, map.address.1 - map.address.0, map.pathname,
					if map.inode != 0 { format!("({})", map.inode) } else { "".into() },
				).as_str()
			);
		}
		let console : Console = lua.globals().get(GLOBAL_CONSOLE)?;
		console.send(out)?;
		Ok(Value::Integer(count))
	}
}

fn thread_maps() -> ProcResult<TasksIter> {
	Ok(Process::myself()?.tasks()?)
}

fn thread_status(task: Result<Task, ProcError>) -> ProcResult<Status> {
	Ok(task?.status()?)
}

pub fn lua_threads(lua: &Lua, ret: Option<bool>) -> Result<Value, Error> {
	let maps = thread_maps()
		.map_err(|e| Error::RuntimeError(
			format!("could not obtain task maps: {}", e)
		))?;
	if ret.unwrap_or(false) {
		let mut out = vec![];
		for task in maps {
			match thread_status(task) {
				Ok(s) => out.push(proc_table(lua, s)?),
				Err(e) => warn!("could not parse task metadata: {}", e),
			}
		}
		Ok(out.to_lua(lua)?)
	} else {
		let mut out = String::new();
		let mut count = 0;
		for task in maps {
			match thread_status(task) {
				Ok(s) => {
					count += 1;
					out.push_str(
						format!(" * [{}] {} {} | {} fd)\n", s.pid, s.state, s.name, s.fdsize).as_str()
					);
				},
				Err(e) => warn!("could not parse task metadata: {}", e),
			}
		}

		let console : Console = lua.globals().get(GLOBAL_CONSOLE)?;
		console.send(out)?;
		Ok(Value::Integer(count))
	}
}
