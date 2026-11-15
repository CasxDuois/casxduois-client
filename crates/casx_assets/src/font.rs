//! Bitmap Font Manager and Unicode Glyph Provider.
//!
//! Supports rendering Unicode text and custom server glyphs (Private Use Area E000-F8FF).

use std::collections::HashMap;

/// Glyph measurement metrics
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphMetrics {
    pub char_code: char,
    pub width: f32,
    pub height: f32,
    pub advance: f32,
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
}

impl GlyphMetrics {
    pub const DEFAULT_HEIGHT: f32 = 8.0;
    pub const DEFAULT_WIDTH: f32 = 6.0;
}

/// Font provider and text measurement
#[derive(Debug, Clone)]
pub struct FontProvider {
    pub glyphs: HashMap<char, GlyphMetrics>,
    pub line_height: f32,
    pub space_advance: f32,
}

impl Default for FontProvider {
    fn default() -> Self {
        let mut provider = Self {
            glyphs: HashMap::new(),
            line_height: 9.0,
            space_advance: 4.0,
        };

        // Populate standard ASCII glyph metrics (ASCII 32..126)
        provider.init_ascii_glyphs();
        provider
    }
}

impl FontProvider {
    pub fn new() -> Self {
        Self::default()
    }

    fn init_ascii_glyphs(&mut self) {
        for c in ' '..='~' {
            let width = match c {
                ' ' => 4.0,
                '!' | '.' | ',' | ':' | ';' | '|' | 'i' | 'l' => 2.0,
                '`' | '\'' => 3.0,
                '[' | ']' | 't' | 'I' => 4.0,
                'f' | 'k' => 5.0,
                '@' | '~' => 7.0,
                _ => 6.0,
            };

            let advance = width + 1.0;
            self.glyphs.insert(
                c,
                GlyphMetrics {
                    char_code: c,
                    width,
                    height: 8.0,
                    advance,
                    u_min: 0.0,
                    v_min: 0.0,
                    u_max: 1.0,
                    v_max: 1.0,
                },
            );
        }
    }

    /// Register a custom glyph (e.g. server icons in PUA \u{E000} - \u{F8FF})
    pub fn register_custom_glyph(&mut self, c: char, width: f32, height: f32, advance: f32) {
        self.glyphs.insert(
            c,
            GlyphMetrics {
                char_code: c,
                width,
                height,
                advance,
                u_min: 0.0,
                v_min: 0.0,
                u_max: 1.0,
                v_max: 1.0,
            },
        );
    }

    /// Retrieve metrics for a character, using a fallback if undefined
    pub fn get_glyph(&self, c: char) -> GlyphMetrics {
        if let Some(glyph) = self.glyphs.get(&c) {
            *glyph
        } else {
            // Fallback for unknown character
            GlyphMetrics {
                char_code: c,
                width: 6.0,
                height: 8.0,
                advance: 7.0,
                u_min: 0.0,
                v_min: 0.0,
                u_max: 1.0,
                v_max: 1.0,
            }
        }
    }

    /// Calculate exact text width in pixels
    pub fn measure_text_width(&self, text: &str) -> f32 {
        let mut width = 0.0;
        let mut chars = text.chars().peekable();

        while let Some(c) = chars.next() {
            // Skip Minecraft formatting codes (§a, §c...)
            if c == '§' {
                chars.next();
                continue;
            }
            let glyph = self.get_glyph(c);
            width += glyph.advance;
        }

        width
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_provider_metrics_and_text_measurement() {
        let mut font = FontProvider::new();
        assert_eq!(font.get_glyph('!').width, 2.0);
        assert_eq!(font.get_glyph('A').width, 6.0);

        let width = font.measure_text_width("Casx");
        assert!(width > 0.0);

        // Verify section code skipping (§a, §c...)
        let raw_width = font.measure_text_width("Hello");
        let colored_width = font.measure_text_width("§cHello");
        assert_eq!(raw_width, colored_width);

        // Register custom glyph for server icon
        font.register_custom_glyph('\u{E001}', 16.0, 16.0, 17.0);
        let custom_glyph = font.get_glyph('\u{E001}');
        assert_eq!(custom_glyph.width, 16.0);
        assert_eq!(custom_glyph.advance, 17.0);
    }
}
