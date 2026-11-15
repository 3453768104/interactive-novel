pub type CmdResult<T> = Result<T, String>;

pub fn cmd_err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// A conflict between the in-app editor buffer and an external editor.
/// Carried as a normal String error with a stable prefix so the UI can react.
pub const CONFLICT_PREFIX: &str = "LORE_CONFLICT:";
