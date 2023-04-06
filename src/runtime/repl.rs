use mlua::{Error, Lua, MultiValue};

use crate::helpers::pretty_lua;

use super::console::Console;

const BS : char = '\u{8}';  // backspace, \b, <BS>, ^B
const FF : char = '\u{C}';  // line feed, <C-L>, ^L
const CMD: char = '\u{1B}'; // ANSI escape char
const CR : char = '\u{A}';  // newline, \n, 10

enum CmdStep {
	Nope,
	One,
	Two,
}

pub struct LuaRepl {
	buffer: String,
	console: Console,
	cmd: CmdStep,
	cmdbuf: u16,
}

impl LuaRepl {
	pub fn new(console: Console) -> Self {
		Self {
			console,
			buffer: String::new(),
			cmd: CmdStep::Nope,
			cmdbuf: 0,
		}
	}

	pub fn buffer(&self) -> String {
		self.buffer.clone()
	}

	pub fn write(&self, txt: String) -> Result<(), Error> {
		self.console.send(txt)
	}

	/// note that errors produced by repl are related to our environment,
	/// all Lua errors will be caught and printed on the console
	pub fn evaluate(&mut self, lua: &Lua, ch: char) -> Result<(), Error> {
		match self.cmd {
			CmdStep::Nope => self.eval(lua, ch)?,
			CmdStep::One => {
				self.cmdbuf |= ch as u16;
				self.cmd = CmdStep::Two;
			}
			CmdStep::Two => {
				self.cmdbuf |= (ch as u16) << 8;

				// TODO parse escape codes? they aren't even all 2 bytes...
				// self.console.send(format!("{}{}{}", BS, BS, BS))?;

				self.cmdbuf = 0;
				self.cmd = CmdStep::Nope;
			},
		}

		Ok(())
	}

	fn eval(&mut self, lua: &Lua, ch: char) -> Result<(), Error> {
		match ch {
			BS => {
				if self.buffer.len() > 0 {
					self.buffer.remove(self.buffer.len() - 1);
				}
			},
			FF => self.console.send(format!("\n@> {}", self.buffer))?,
			CMD => self.cmd = CmdStep::One,
			CR => {
				match lua.load(&self.buffer).eval::<MultiValue>() {
					Ok(values) => {
						let mut once = false;
						for val in values {
							once = true;
							self.console.send(
								format!("=({}) {}", val.type_name(), pretty_lua(val))
							)?;
						}
						self.console.send(format!("{}@> ", if once { "\n" } else { "" }))?;
						self.buffer = String::new();
					},
					Err(e) => {
						match e {
							mlua::Error::SyntaxError {
								message: _,
								incomplete_input: true
							} => {
								self.console.send("@    ".into())?;
								self.buffer.push(ch);
							},
							_ => {
								self.console.send(format!("! {}\n@> ", e))?;
								self.buffer = String::new();
							},
						}
					}
				}
			},
			'\0' => return Err(Error::RuntimeError("null byte in stream".into())),
			_ => self.buffer.push(ch),
		}
		Ok(())
	}
}
