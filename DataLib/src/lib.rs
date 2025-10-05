pub mod types;
pub mod util;
pub mod sde;
pub mod hardcoded;
#[cfg(test)]
pub mod test;

pub const CRATE_NAME: &'static str = env!("CARGO_PKG_NAME");
pub const CRATE_VERSION: &'static str = env!("CARGO_PKG_VERSION");
pub const CRATE_REPO: &'static str = env!("CARGO_PKG_REPOSITORY");