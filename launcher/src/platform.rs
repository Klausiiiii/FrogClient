//! Platform-specific bits; everything else in the launcher stays OS-neutral.

#[cfg(windows)]
pub const CLASSPATH_SEP: &str = ";";
#[cfg(not(windows))]
pub const CLASSPATH_SEP: &str = ":";
