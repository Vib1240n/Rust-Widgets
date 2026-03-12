use serde::{Deserialize, Serialize};

/// Widget positioning on screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
    TopLeft,
    TopCenter,
    #[default]
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    Center,
    CenterLeft,
    CenterRight,
}

impl Position {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "top-left" | "topleft" => Position::TopLeft,
            "top-center" | "topcenter" | "top" => Position::TopCenter,
            "top-right" | "topright" => Position::TopRight,
            "bottom-left" | "bottomleft" => Position::BottomLeft,
            "bottom-center" | "bottomcenter" | "bottom" => Position::BottomCenter,
            "bottom-right" | "bottomright" => Position::BottomRight,
            "center" => Position::Center,
            "center-left" | "centerleft" | "left" => Position::CenterLeft,
            "center-right" | "centerright" | "right" => Position::CenterRight,
            _ => Position::TopRight,
        }
    }
}
