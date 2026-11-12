//! The brand this press prints in: Wolven Tech. The mark is a wolf howling on
//! a rock between two pines under the moon, drawn in `currentColor`; the
//! palette is a monochrome night forest with one warm note (the moon).
//! To print in a house style, change these constants and rebuild: a story
//! cannot restyle the page. `brand_tests.rs` holds the contrast floors.

pub struct Brand {
    /// Shown beside the mark when a story sets no `publisher:`.
    pub name: &'static str,
    /// Inline SVG; `currentColor` parts follow the ink, so it flips in dark mode.
    pub logo: Option<&'static str>,
    pub ink: &'static str,
    /// Light-mode accent: link underlines, kickers, the title's last word.
    pub accent: &'static str,
    /// The accent on the dark paper, where `accent` would be too dim to read.
    pub accent_dark: &'static str,
    /// Text set on an `accent` fill.
    pub on_accent: &'static str,
    pub highlight: &'static str,
}

pub const BRAND: Brand = Brand {
    name: "Wolven Tech",
    logo: Some(LOGO),
    ink: "#16191d",
    accent: "#5f6b76",
    accent_dark: "#b8c0c8",
    on_accent: "#ffffff",
    highlight: "#c9a95c",
};

const LOGO: &str = r##"<svg class="logo" viewBox="0 0 64 64" role="img" aria-label="Wolven Tech" xmlns="http://www.w3.org/2000/svg" fill="currentColor"><circle cx="32" cy="32" r="29.5" fill="none" stroke="currentColor" stroke-width="3"/><circle cx="46" cy="11.5" r="3.6" opacity=".45"/><path d="M8 50H56A30 30 0 0 1 8 50Z"/><path d="M18 51Q23 45.4 32 45.4Q41 45.4 46 51Z"/><path d="M49 18L45.5 28H47.5L44 37H46.5L43 46H48V50H50V46H55L51.5 37H54L50.5 28H52.5Z"/><path d="M16 20L13 29H14.5L11 38H13L10 46H15V50H17V46H22L19 38H21L17.5 29H19Z"/><path transform="translate(0 1.2)" d="M20 43.2C21.5 40.5 23.5 39.5 25.5 39C26.5 35 27.5 31.5 29.5 29C30.5 27 31 24.5 31.8 21.5L31.5 16.8L33.8 19.6C34.5 18.8 35.2 18 36 17.3L40.4 13.2L41.4 14.4L38.2 17.4L40.4 17.2L40.4 18.3C38.5 19.4 37 20.2 36.2 21.2C35.8 23.5 36.2 26 37 28.5C37.4 30.5 37.3 32 37.2 33.5L37.6 43L39.2 43.4V44.4H35.8L35.6 37.5C34.4 38.5 33.4 40 33.6 42.4L35 43.4V44.4H26C24 44.4 21.8 44.2 20 43.2Z"/></svg>"##;
