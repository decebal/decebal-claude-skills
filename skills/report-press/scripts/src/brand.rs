//! The brand this press prints in: a neutral newsprint palette and no logo.
//! The masthead shows the story's `publisher:` as a text wordmark instead.
//! To print in a house style, change these constants and rebuild: a story
//! cannot restyle the page.

pub struct Brand {
    pub name: &'static str,
    /// Inline SVG; `currentColor` parts follow the ink, so it flips in dark mode.
    pub logo: Option<&'static str>,
    pub ink: &'static str,
    pub accent: &'static str,
    pub highlight: &'static str,
}

pub const BRAND: Brand = Brand {
    name: "",
    logo: None,
    ink: "#1c2733",
    accent: "#b4372b",
    highlight: "#d4a017",
};
