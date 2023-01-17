// #![allow(unused)]

use nix::errno::Errno;
use nix::sys::ptrace;
use nix::sys::ptrace::Request;
use nix::sys::signal;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use std::os::unix::process::CommandExt;
use std::process::Child;
use std::process::Command;
use std::{mem, ptr};

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
    pub fn new(target: &str, args: &Vec<String>) -> Option<Inferior> {
        let mut cmd = Command::new(target);
        unsafe {
            cmd.pre_exec(child_traceme);
        }
        println!("{} and {:?}", target, args);
        let child = match cmd.args(args).spawn() {
            Ok(c) => c,
            Err(e) => panic!("{}", e),
        };
        let inferior = Inferior { child };
        match waitpid(inferior.pid(), None) {
            Ok(status) => {
                println!("{:?}", status);
                Some(inferior)
            }
            Err(e) => {
                println!("{:?}", e);
                None
            }
        }

        // let status = inferior.wait(None).ok()?;
        // println!("{:?}", status);
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
            WaitStatus::Signaled(_pid, signal, _core_dumped) => Status::Signaled(signal),
            WaitStatus::Stopped(_pid, signal) => {
                // let regs = ptrace::getregs(self.pid())?;
                Status::Stopped(signal, 0)
            }
            other => panic!("waitpid returned unexpected status: {:?}", other),
        })
    }

    pub fn cont(&self) -> Result<Status, nix::Error> {
        ptrace::cont(self.pid(), None)?;
        self.wait(None)
    }

    pub fn kill(&mut self) -> Result<Status, nix::Error> {
        if self.child.kill().is_ok() {
            return self.wait(None);
        };
        panic!("Cannot kill inferior")
    }

    pub fn print_backtrace(&self) -> Result<(), nix::Error> {
        // unsafe {
        //     libc::ptrace(lib::, self.pid());
        // }
        // match ptrace::getregs(self.pid()) {
        //     Ok(regs) => println!("{:?}", regs),
        //     Err(e) => println!("{}", e),
        // }
        Ok(())
    }
}

// //

// fn ptrace_get_data<T>(request: Request, pid: Pid) -> nix::Result<T> {
//     let mut data = mem::MaybeUninit::uninit();
//     let res = unsafe {
//         libc::ptrace(
//             request as ptrace::RequestType,
//             libc::pid_t::from(pid),
//             ptr::null_mut::<T>(),
//             data.as_mut_ptr() as *const _ as *const c_void,
//         )
//     };
//     Errno::result(res)?;
//     Ok(unsafe { data.assume_init() })
// }

// pub fn setregs(pid: Pid, regs: user_regs_struct) -> nix::Result<()> {
//     let res = unsafe {
//         libc::ptrace(
//             Request::PTRACE_SETREGS as ptrace::RequestType,
//             libc::pid_t::from(pid),
//             ptr::null_mut::<c_void>(),
//             &regs as *const _ as *const c_void,
//         )
//     };
//     Errno::result(res).map(drop)
// }

// pub fn getregs(pid: Pid) -> nix::Result<user_regs_struct> {
//     ptrace_get_data::<user_regs_struct>(Request::PTRACE_GETREGS, pid)
// }
