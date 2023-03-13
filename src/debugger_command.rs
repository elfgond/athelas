pub enum DebuggerCommand {
    Quit,
    Run(Vec<String>),
    Cont, // continue
    Backtrace,
    Break(String),
}

impl DebuggerCommand {
    pub fn from_tokens(tokens: &[&str]) -> Option<DebuggerCommand> {
        match tokens[0] {
            "q" | "quit" => Some(DebuggerCommand::Quit),
            "r" | "run" => {
                let args = tokens[1..].to_vec();
                Some(DebuggerCommand::Run(
                    args.iter().map(|s| s.to_string()).collect(),
                ))
            }
            "c" | "cont" | "continue" => Some(DebuggerCommand::Cont),
            "bt" | "back" | "backtrace" => Some(DebuggerCommand::Backtrace),
            "b" | "brk" | "break" => {
                let arg = tokens[1..].join(" ");
                Some(DebuggerCommand::Break(arg))
            }
            // Default case:
            _ => None,
        }
    }

    /// Returns `true` if the debugger command is [`Break`].
    ///
    /// [`Break`]: DebuggerCommand::Break
    #[allow(dead_code)]
    #[must_use]
    pub fn is_break(&self) -> bool {
        matches!(self, Self::Break(..))
    }
}
