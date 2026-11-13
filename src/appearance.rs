use eframe::egui::{self, Color32};
use egui_term::{ColorPalette, TerminalTheme};

pub struct Appearance {
    pub background: Color32,
    pub sidebar: Color32,
    pub chrome: Color32,
    pub separator: Color32,
    pub selected: Color32,
    pub muted: Color32,
    pub control: Color32,
    pub ssh: Color32,
    pub matte: fn(Color32) -> Color32,
    pub terminal: TerminalTheme,
}

impl Appearance {
    pub fn for_visuals(visuals: &egui::Visuals) -> Self {
        let dark = visuals.dark_mode;
        let palette = if dark {
            ColorPalette { background: "#27292b".into(), foreground: "#ebebeb".into(), ..ColorPalette::default() }
        } else {
            ColorPalette {
                foreground: "#24292f".into(),
                background: "#ffffff".into(),
                black: "#24292f".into(),
                red: "#a41927".into(),
                green: "#116329".into(),
                yellow: "#785400".into(),
                blue: "#0550ae".into(),
                magenta: "#6639ba".into(),
                cyan: "#086b72".into(),
                white: "#6e7781".into(),
                bright_black: "#57606a".into(),
                bright_red: "#b62324".into(),
                bright_green: "#1a7f37".into(),
                bright_yellow: "#806000".into(),
                bright_blue: "#0969da".into(),
                bright_magenta: "#8250df".into(),
                bright_cyan: "#087780".into(),
                bright_white: "#57606a".into(),
                bright_foreground: Some("#161b22".into()),
                dim_foreground: "#57606a".into(),
                dim_black: "#57606a".into(),
                dim_red: "#8c2730".into(),
                dim_green: "#305f3a".into(),
                dim_yellow: "#705a20".into(),
                dim_blue: "#345d8b".into(),
                dim_magenta: "#654888".into(),
                dim_cyan: "#34656a".into(),
                dim_white: "#57606a".into(),
            }
        };
        Self {
            background: if dark { Color32::from_rgb(39, 41, 43) } else { Color32::WHITE },
            sidebar: if dark { Color32::from_rgb(48, 48, 47) } else { Color32::from_rgb(233, 232, 231) },
            chrome: if dark { Color32::from_rgb(54, 55, 53) } else { Color32::from_rgb(240, 240, 239) },
            separator: if dark { Color32::from_rgb(76, 78, 75) } else { Color32::from_rgb(215, 215, 214) },
            selected: if dark { Color32::from_rgb(76, 78, 76) } else { Color32::from_rgb(211, 211, 210) },
            muted: if dark { Color32::from_rgb(186, 187, 185) } else { Color32::from_rgb(102, 105, 111) },
            control: if dark { Color32::from_rgb(74,76,75) } else { Color32::from_rgb(225,225,223) },
            ssh: if dark { Color32::from_rgb(132,186,243) } else { Color32::from_rgb(33,105,175) },
            matte: if dark { matte_dark } else { matte_light },
            terminal: TerminalTheme::new(Box::new(palette)),
        }
    }
}

/// sRGB grayscale plus the approved neutral veil. No blur, layer or animation.
fn matte(color: Color32, gray: u32, opacity: u32) -> Color32 {
    let [r,g,b,alpha] = color.to_srgba_unmultiplied();
    let luminance = (2126 * r as u32 + 7152 * g as u32 + 722 * b as u32 + 5000) / 10000;
    let value = ((luminance * (100 - opacity) + gray * opacity + 50) / 100) as u8;
    Color32::from_rgba_unmultiplied(value, value, value, alpha)
}
pub fn matte_light(color: Color32) -> Color32 { matte(color, 128, 24) }
pub fn matte_dark(color: Color32) -> Color32 { matte(color, 87, 26) }

impl Appearance {
    pub fn badge_colors(&self, ssh: bool, active: bool) -> (Color32, Color32) {
        let foreground=if ssh {self.ssh} else {self.muted};
        let background=if ssh {
            let mix=|a:u8,b:u8| ((a as u32 * 13 + b as u32 * 87 + 50) / 100) as u8;
            Color32::from_rgb(mix(self.ssh.r(),self.chrome.r()),mix(self.ssh.g(),self.chrome.g()),mix(self.ssh.b(),self.chrome.b()))
        } else {self.control};
        if active {(foreground,background)} else {((self.matte)(foreground),(self.matte)(background))}
    }

    pub fn apply_style(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(7.0, 4.0);
        style.spacing.interact_size.y = 26.0;
        style.visuals.panel_fill = self.chrome;
        style.visuals.window_fill = self.chrome;
        style.visuals.selection.bg_fill = self.selected;
        style.visuals.selection.stroke.color = style.visuals.text_color();
        style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        style.visuals.widgets.hovered.weak_bg_fill = self.selected;
        style.visuals.widgets.active.weak_bg_fill = self.selected;
        style.visuals.widgets.noninteractive.bg_stroke.color = self.separator;
        style.animation_time = 0.0;
        style.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
        style.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        ctx.set_style(style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matte_does_not_apply_dim_text_alpha_twice() {
        let dim = Color32::from_rgba_unmultiplied(200,200,200,128);
        let muted = matte_light(dim);
        assert_eq!(muted.a(), 128);
        let [gray,_,_,_] = muted.to_srgba_unmultiplied();
        assert!((180..=185).contains(&gray), "{gray}");
    }
    #[test]
    fn matte_is_neutral_and_preserves_readable_light_and_dark_surfaces() {
        let light = matte_light(Color32::WHITE);
        let dark = matte_dark(Color32::from_rgb(39, 41, 43));
        assert_eq!(light, Color32::from_gray(225));
        assert_eq!(dark.r(), dark.g());
        assert_eq!(dark.g(), dark.b());
        assert!(dark.r() >= 50 && dark.r() <= 54);
        for color in [Color32::RED, Color32::GREEN, Color32::BLUE] {
            let muted = matte_dark(color);
            assert_eq!(muted.r(), muted.g());
            assert_eq!(muted.g(), muted.b());
        }
    }
}
