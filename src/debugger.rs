use std::process::exit;

use crate::debugger_command::DebuggerCommand;
use crate::dwarf_data::DwarfData;
use crate::inferior::Inferior;
use rustyline::error::ReadlineError;
use rustyline::Editor;

pub struct Debugger {
    target: String,
    history_path: String,
    readline: Editor<()>,
    inferior: Option<Inferior>,
    debug_data: DwarfData,
}

impl Debugger {
    /// Initializes the debugger.
    pub fn new(target: &str) -> Debugger {
        // TODO (milestone 3): initialize the DwarfData
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
        }
    }

    pub fn run(&mut self) {
        loop {
            match self.get_next_command() {
                DebuggerCommand::Run(args) => match &mut self.inferior {
                    Some(inferior) => match inferior.kill() {
                        Ok(status) => {
                            println!("{status:?}");
                            self.start_deet(args);
                        }
                        Err(e) => println!("could not kill previous child {e:?}"),
                    },
                    None => self.start_deet(args),
                },
                DebuggerCommand::Cont => {
                    if let Some(inferior) = &self.inferior {
                        match inferior.cont() {
                            Ok(status) => {
                                println!("Child process {status:?}")
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
            }
        }
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
        if let Some(inferior) = Inferior::new(&self.target, &args) {
            // Create the inferior
            self.inferior = Some(inferior);
            // TODO (milestone 1): make the inferior run
            match self.inferior.as_mut().unwrap().cont() {
                Ok(status) => println!("Child {status:?}"),
                Err(_) => panic!("Error continuing program"),
            }
        } else {
            println!("Error starting subprocess");
        }
    }
}
