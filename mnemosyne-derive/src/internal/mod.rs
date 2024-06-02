use self::symbol::Symbol;

pub mod getter;
pub mod symbol;

pub struct AttributeArgs {
    pub directive: Option<String>,
    pub state: Option<String>,
}

// Attributes
pub const COMMAND_ATTRIBUTE: &str = "command";
pub const EVENT_ATTRIBUTE: &str = "event";

// Symbols
pub const DIRECTIVE: Symbol = Symbol("directive");
pub const STATE: Symbol = Symbol("state");
