use std::process::exit;

use crate::debugger_command::DebuggerCommand;
use crate::dwarf_data::DwarfData;
use crate::inferior::{Inferior, Status};
use rustyline::error::ReadlineError;
use rustyline::Editor;

pub struct Debugger {
    target: String,
    history_path: String,
    readline: Editor<()>,
    inferior: Option<Inferior>,
    debug_data: DwarfData,
    breakpoints: Vec<usize>,
}

impl Debugger {
    /// Initializes the debugger.
    pub fn new(target: &str) -> Debugger {
        let debug_data = match DwarfData::from_file(target) {
            Ok(val) => val,
            Err(crate::dwarf_data::Error::ErrorOpeningFile) => {
                println!("Could not open file {target}");
                exit(1);
            }
            Err(crate::dwarf_data::Error::DwarfFormatError(err)) => {
                println!("could not open file {target}: {err:?}");
                exit(1);
            }
        };
        debug_data.print();
        let history_path = format!("{}/.deet_history", std::env::var("HOME").unwrap());
        let mut readline = Editor::<()>::new();
        // Attempt to load history from ~/.deet_history if it exists
        let _ = readline.load_history(&history_path);

        Debugger {
            target: target.to_string(),
            history_path,
            readline,
            inferior: None,
            debug_data,
            breakpoints: vec![],
        }
    }

    pub fn run(&mut self) {
        loop {
            match self.get_next_command() {
                DebuggerCommand::Run(args) => match &mut self.inferior {
                    // if self.inferior.is_some() {}
                    Some(inferior) => match inferior.kill() {
                        Ok(status) => {
                            match status {
                                Status::Exited(exit_code) => {
                                    println!("Child exited (status {exit_code})");
                                    self.inferior = None;
                                }
                                Status::Signaled(signal) => {
                                    println!("Child exited due to signal {signal}");
                                    self.inferior = None;
                                }
                                Status::Stopped(signal, rip) => {
                                    println!("Child Stopped ({signal:?}, {rip})");
                                    let line = self.debug_data.get_line_from_addr(rip);
                                    let func = self.debug_data.get_function_from_addr(rip);
                                    if line.is_some() && func.is_some() {
                                        println!(
                                            "Stopped at {} ({})",
                                            func.unwrap(),
                                            line.unwrap()
                                        );
                                    }
                                }
                            }
                            self.inferior = None;
                            self.start_deet(args);
                        }
                        Err(_) => self.start_deet(args),
                    },
                    None => self.start_deet(args),
                },
                DebuggerCommand::Cont => {
                    if let Some(inferior) = &self.inferior {
                        match inferior.cont() {
                            Ok(status) => {
                                println!("Child process {status:?}");
                                if let Status::Stopped(_, rip) = status {
                                    let line = self.debug_data.get_line_from_addr(rip);
                                    let func = self.debug_data.get_function_from_addr(rip);
                                    if line.is_some() && func.is_some() {
                                        println!(
                                            "Stopped at {} ({})",
                                            func.unwrap(),
                                            line.unwrap()
                                        );
                                    }
                                }
                            }
                            Err(e) => println!("error cannot continue child process: {e}"),
                        }
                    }
                }
                DebuggerCommand::Quit => {
                    if let Some(inferior) = &mut self.inferior {
                        match inferior.kill() {
                            Ok(status) => {
                                println!("exiting inferior {status:?}");
                            }
                            Err(e) => println!("could not kill previous inferior {e:?}"),
                        }
                    };
                    return;
                }
                DebuggerCommand::Backtrace => {
                    if let Some(inferior) = &self.inferior {
                        inferior.print_backtrace(&self.debug_data).unwrap()
                    }
                }
                DebuggerCommand::Break(arg) => {
                    let addr = self.parse_address(&arg[1..]).unwrap();
                    self.breakpoints.push(addr);
                    // check if inferior is running already and borrow as mutable reference
                    if self.inferior.is_some() {
                        let inf = self.inferior.as_mut().unwrap();
                        inf.set_breakpoint(addr).unwrap();
                    }
                    // The ref mut part of the pattern means that inferior is a mutable reference to the value inside the Some variant,
                    // rather than taking ownership of the value.
                    // if let Some(ref mut inferior) = self.inferior {
                    //     inferior.set_breakpoint(addr).unwrap();
                    // }
                }
            }
        }
    }

    #[allow(dead_code)]
    fn parse_address(&self, addr: &str) -> Option<usize> {
        let addr_without_0x = if addr.to_lowercase().starts_with("0x") {
            &addr[2..]
        } else {
            addr
        };
        usize::from_str_radix(addr_without_0x, 16).ok()
    }

    /// This function prompts the user to enter a command, and continues re-prompting until the user
    /// enters a valid command. It uses DebuggerCommand::from_tokens to do the command parsing.
    ///
    /// You don't need to read, understand, or modify this function.
    fn get_next_command(&mut self) -> DebuggerCommand {
        loop {
            // Print prompt and get next line of user input
            match self.readline.readline("(deet) ") {
                Err(ReadlineError::Interrupted) => {
                    // User pressed ctrl+c. We're going to ignore it
                    println!("Type \"quit\" to exit");
                }
                Err(ReadlineError::Eof) => {
                    // User pressed ctrl+d, which is the equivalent of "quit" for our purposes
                    return DebuggerCommand::Quit;
                }
                Err(err) => {
                    panic!("Unexpected I/O error: {:?}", err);
                }
                Ok(line) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    self.readline.add_history_entry(line.as_str());
                    if let Err(err) = self.readline.save_history(&self.history_path) {
                        println!(
                            "Warning: failed to save history file at {}: {}",
                            self.history_path, err
                        );
                    }
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if let Some(cmd) = DebuggerCommand::from_tokens(&tokens) {
                        return cmd;
                    } else {
                        println!("Unrecognized command.");
                    }
                }
            }
        }
    }

    fn start_deet(&mut self, args: Vec<String>) {
        if let Some(inferior) = Inferior::new(&self.target, &args, &self.breakpoints) {
            match inferior.cont() {
                Ok(status) => match status {
                    Status::Exited(exit_code) => {
                        println!("Child exited (status {exit_code})");
                        self.inferior = None;
                    }
                    Status::Signaled(signal) => {
                        println!("Child exited due to signal {signal}");
                        self.inferior = None;
                    }
                    Status::Stopped(signal, rip) => {
                        println!("Child Stopped ({signal:?}, {rip})");
                        let line = self.debug_data.get_line_from_addr(rip);
                        let func = self.debug_data.get_function_from_addr(rip);
                        if line.is_some() && func.is_some() {
                            println!("Stopped at {} ({})", func.unwrap(), line.unwrap());
                        }
                        self.inferior = Some(inferior);
                    }
                },
                Err(_) => panic!("Error continuing program"),
            }
        } else {
            println!("Error starting subprocess");
        }
    }
}
