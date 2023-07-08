use std::{sync::atomic::{AtomicBool, Ordering}, ffi::c_int};

use mlua::{Lua, Error};
use nix::sys::signal::{SigHandler, Signal::SIGSEGV};

const SIGSEGV_HOOK : AtomicBool = AtomicBool::new(false);

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

