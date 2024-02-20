# Cordy
A remote control framework for processes.
Inject lua code into running processes, or just mess around with the REPL.

## Usage
Cordy is a shared object which you need to inject into running processes (consider using my [pox framework](https://git.alemi.dev/pox.git/about) or [dll-syringe](https://github.com/OpenByteDev/dll-syringe)).

Once a process is infected, a new thread will be spawned inside with a tokio event loop. A socket on localhost will be opened on port 13337 and you can just connect with netcat and access the REPL.

Some builtin functions are added to the Lua REPL to help with messing around:

```
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
```

It's possible to load lua scripts and programmatically take actions, but no automated way is implemented yet (must connect to the repl and require your script)

There are no handrails: be aware of race conditions or segfaults!

## Status
Cordy is still in development. I've built this to explore running processes, dynamic loading and the heap. I don't think this has malicious uses since, if you loaded your shared object, you basically already owned the process. If you think otherwise let me know!

## Name
Named from [Ophiocordyceps_unilateralis](https://en.wikipedia.org/wiki/Ophiocordyceps_unilateralis) since this kind of zombifies processes.
