// #![allow(unused)]

use nix::sys::ptrace;
use nix::sys::signal;
use nix::sys::signal::Signal::SIGTRAP;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use std::mem::size_of;
use std::os::unix::process::CommandExt;
use std::process::Child;
use std::process::Command;

use crate::dwarf_data::DwarfData;

#[derive(Debug)]
pub enum Status {
    /// Indicates inferior stopped. Contains the signal that stopped the process, as well as the
    /// current instruction pointer that it is stopped at.
    Stopped(signal::Signal, usize),

    /// Indicates inferior exited normally. Contains the exit status code.
    Exited(i32),

    /// Indicates the inferior exited due to a signal. Contains the signal that killed the
    /// process.
    Signaled(signal::Signal),
}

/// This function calls ptrace with PTRACE_TRACEME to enable debugging on a process. You should use
/// pre_exec with Command to call this in the child process.
fn child_traceme() -> Result<(), std::io::Error> {
    ptrace::traceme()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "ptrace TRACEME failed"))
}

pub struct Inferior {
    child: Child,
}

impl Inferior {
    /// Attempts to start a new inferior process. Returns Some(Inferior) if successful, or None if
    /// an error is encountered.
    pub fn new(target: &str, args: &Vec<String>, breakpoints: &[usize]) -> Option<Inferior> {
        let mut cmd = Command::new(target);
        unsafe {
            cmd.pre_exec(child_traceme);
        }
        let child = match cmd.args(args).spawn() {
            Ok(c) => c,
            Err(e) => panic!("{}", e),
        };
        let mut inferior = Inferior { child };
        match inferior.wait(None) {
            Ok(status) => {
                if let Status::Stopped(sig, _) = status {
                    if sig == SIGTRAP {
                        println!("{sig} detected. setting breakpoints if any...");
                        for addr in breakpoints {
                            inferior.set_breakpoint(*addr).unwrap();
                        }
                    }
                }
                Some(inferior)
            }
            Err(e) => {
                println!("E: {e:?}");
                None
            }
        }
    }

    pub fn set_breakpoint(&mut self, addr: usize) -> Result<u8, nix::Error> {
        self.write_byte(addr, 0xcc)
    }

    /// Returns the pid of this inferior.
    pub fn pid(&self) -> Pid {
        nix::unistd::Pid::from_raw(self.child.id() as i32)
    }

    /// Calls waitpid on this inferior and returns a Status to indicate the state of the process
    /// after the waitpid call.
    pub fn wait(&self, options: Option<WaitPidFlag>) -> Result<Status, nix::Error> {
        Ok(match waitpid(self.pid(), options)? {
            WaitStatus::Exited(_pid, exit_code) => Status::Exited(exit_code),
            WaitStatus::Signaled(_pid, signal, _core_dumped) => {
                if signal == SIGTRAP {
                    println!("SIGTRAP DETECTED");
                }
                Status::Signaled(signal)
            }
            WaitStatus::Stopped(_pid, signal) => {
                let regs = ptrace::getregs(self.pid())?;
                Status::Stopped(signal, regs.rip as usize)
            }
            WaitStatus::PtraceEvent(_pid, signal, _core_dumped) => {
                println!("{signal} detected");
                Status::Signaled(signal)
            }
            other => panic!("waitpid returned unexpected status: {:?}", other),
        })
    }

    pub fn cont(&self) -> Result<Status, nix::Error> {
        ptrace::cont(self.pid(), None)?;
        self.wait(None)
    }

    pub fn kill(&mut self) -> Result<Status, nix::Error> {
        println!("killing running inferior (pid {})", self.pid());
        match self.child.kill() {
            Ok(_) => self.wait(None),
            _ => Err(nix::Error::ECHILD),
        }
    }

    pub fn print_backtrace(&self, debug_data: &DwarfData) -> Result<(), nix::Error> {
        let regs = ptrace::getregs(self.pid())?;
        let mut current_stack_frame = regs.rbp as usize;
        // let stack_pointer = regs.rsp as usize;
        let mut instruction_ptr = regs.rip as usize;
        loop {
            let func_name = debug_data.get_function_from_addr(instruction_ptr);
            match &func_name {
                Some(f) => {
                    println!(
                        "{} ({})",
                        f,
                        debug_data.get_line_from_addr(instruction_ptr).unwrap() // danger of panicing at runtime
                    );
                }
                None => break,
            }
            if let Some(func_name) = func_name {
                if func_name == "main" {
                    break;
                }
            }
            instruction_ptr =
                ptrace::read(self.pid(), (current_stack_frame + 8) as ptrace::AddressType)?
                    as usize;
            current_stack_frame =
                ptrace::read(self.pid(), current_stack_frame as ptrace::AddressType)? as usize;
        }
        Ok(())
    }

    fn align_addr_to_word(&self, addr: usize) -> usize {
        addr & (-(size_of::<usize>() as isize) as usize)
        // println!(
        //     "{aligned}: [{}][{}]",
        //     size_of::<usize>() as isize,
        //     (-(size_of::<usize>() as isize) as usize)
        // );
    }

    // In order to write a byte, you must read a full 8 bytes into a long,
    // use bitwise arithmetic to substitute the desired byte into that long,
    // and then write the full long back to the child’s memory.
    // Additionally, despite the nix crate’s ptrace having a much nicer interface than the ptrace syscall,
    // it’s still a bit funky to use (it requires some bizarre type conversions).
    fn write_byte(&mut self, addr: usize, val: u8) -> Result<u8, nix::Error> {
        let aligned_addr = self.align_addr_to_word(addr);
        let byte_offset = addr - aligned_addr;
        let word = ptrace::read(self.pid(), aligned_addr as ptrace::AddressType)? as u64;
        let orig_byte = (word >> (8 * byte_offset)) & 0xff;
        let masked_word = word & !(0xff << (8 * byte_offset));
        let updated = masked_word | ((val as u64) << (8 * byte_offset));
        unsafe {
            ptrace::write(
                self.pid(),
                aligned_addr as ptrace::AddressType,
                updated as *mut std::ffi::c_void,
            )?;
        }
        Ok(orig_byte as u8)
    }
}
