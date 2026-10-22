//! Options for "Save as PDF…", shared by the platform exporters
//! (WebView2 `PrintToPdf` on Windows, WKWebView print operation on macOS).

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfOptions {
    /// Page size in millimetres (already oriented).
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub margin_mm: f64,
    pub landscape: bool,
    pub backgrounds: bool,
    pub header_footer: bool,
    pub title: String,
}

pub const MM_PER_INCH: f64 = 25.4;
