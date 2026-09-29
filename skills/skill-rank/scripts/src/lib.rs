//! Rate every installed Claude Code and Codex skill with `TypeSafe` Jev, keep
//! the best-rated ones inside each host's skill-listing budget, and search all
//! of them ranked by rating.

pub mod apply;
pub mod catalog;
pub mod cli;
pub mod fmt;
pub mod frontmatter;
pub mod jev;
pub mod rate;
pub mod scan;
pub mod search;
pub mod usage;

#[cfg(test)]
mod test_support;
