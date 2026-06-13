use ratatui::style::Color;

const COLOR_MODE_ENV: &str = "PI_SWITCH_COLOR_MODE";

pub const DRACULA_GREEN: (u8, u8, u8) = (80, 250, 123);
pub const DRACULA_CYAN: (u8, u8, u8) = (139, 233, 253);
pub const DRACULA_YELLOW: (u8, u8, u8) = (241, 250, 140);
pub const DRACULA_RED: (u8, u8, u8) = (255, 85, 85);
pub const DRACULA_COMMENT: (u8, u8, u8) = (98, 114, 164);
pub const DRACULA_SURFACE: (u8, u8, u8) = (68, 71, 90);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    NoColor,
    TrueColor,
    Ansi256,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub accent: Color,
    pub ok: Color,
    pub warn: Color,
    pub err: Color,
    pub dim: Color,
    pub cyan: Color,
    pub surface: Color,
    pub no_color: bool,
}

pub fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

fn parse_color_mode(value: &str) -> Option<ColorMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => None,
        "none" | "no-color" => Some(ColorMode::NoColor),
        "rgb" | "truecolor" | "24bit" | "24-bit" => Some(ColorMode::TrueColor),
        "ansi256" | "ansi-256" | "256" | "256color" | "256-color" => Some(ColorMode::Ansi256),
        _ => None,
    }
}

fn color_mode_override() -> Option<ColorMode> {
    parse_color_mode(&std::env::var(COLOR_MODE_ENV).ok()?)
}

fn env_supports_truecolor(key: &str) -> bool {
    std::env::var(key)
        .map(|value| {
            let normalized = value.to_ascii_lowercase();
            normalized.contains("truecolor")
                || normalized.contains("24bit")
                || normalized.contains("24-bit")
                || normalized.contains("-direct")
                || normalized.ends_with("direct")
        })
        .unwrap_or(false)
}

fn known_ansi256_terminal() -> bool {
    std::env::var("TERM_PROGRAM")
        .map(|value| value == "Apple_Terminal")
        .unwrap_or(false)
}

fn ssh_plain_xterm_prefers_ansi256() -> bool {
    std::env::var_os("SSH_TTY").is_some()
        && std::env::var("TERM")
            .map(|value| value.eq_ignore_ascii_case("xterm"))
            .unwrap_or(false)
}

fn detected_color_mode() -> ColorMode {
    if no_color() {
        return ColorMode::NoColor;
    }
    if let Some(mode) = color_mode_override() {
        return mode;
    }
    if known_ansi256_terminal() {
        return ColorMode::Ansi256;
    }
    if env_supports_truecolor("COLORTERM") || env_supports_truecolor("TERM") {
        return ColorMode::TrueColor;
    }
    if ssh_plain_xterm_prefers_ansi256() {
        return ColorMode::Ansi256;
    }
    ColorMode::TrueColor
}

fn cube_index(value: u8) -> u8 {
    match value {
        0..=47 => 0,
        48..=114 => 1,
        _ => ((value - 35) / 40).min(5),
    }
}

fn cube_level(index: u8) -> u8 {
    [0, 95, 135, 175, 215, 255][index as usize]
}

fn ansi256_cube(r: u8, g: u8, b: u8) -> (u8, u8, u8, u8) {
    let ri = cube_index(r);
    let gi = cube_index(g);
    let bi = cube_index(b);
    (
        16 + (36 * ri) + (6 * gi) + bi,
        cube_level(ri),
        cube_level(gi),
        cube_level(bi),
    )
}

fn ansi256_gray(r: u8, g: u8, b: u8) -> (u8, u8, u8, u8) {
    let avg = ((r as u16 + g as u16 + b as u16) / 3) as u8;
    let index = if avg <= 8 {
        0
    } else if avg >= 238 {
        23
    } else {
        (((avg as u16 - 8 + 5) / 10) as u8).min(23)
    };
    let level = 8 + index * 10;
    (232 + index, level, level, level)
}

fn color_distance_sq(lhs: (u8, u8, u8), rhs: (u8, u8, u8)) -> u32 {
    let dr = lhs.0 as i32 - rhs.0 as i32;
    let dg = lhs.1 as i32 - rhs.1 as i32;
    let db = lhs.2 as i32 - rhs.2 as i32;
    (dr * dr + dg * dg + db * db) as u32
}

fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    let source = (r, g, b);
    let cube = ansi256_cube(r, g, b);
    let gray = ansi256_gray(r, g, b);

    let cube_distance = color_distance_sq(source, (cube.1, cube.2, cube.3));
    let gray_distance = color_distance_sq(source, (gray.1, gray.2, gray.3));

    if cube_distance <= gray_distance {
        cube.0
    } else {
        gray.0
    }
}

fn terminal_color(color_mode: ColorMode, rgb: (u8, u8, u8)) -> Color {
    match color_mode {
        ColorMode::NoColor => Color::Reset,
        ColorMode::TrueColor => Color::Rgb(rgb.0, rgb.1, rgb.2),
        ColorMode::Ansi256 => Color::Indexed(rgb_to_ansi256(rgb.0, rgb.1, rgb.2)),
    }
}

pub fn palette_color(rgb: (u8, u8, u8)) -> Color {
    terminal_color(detected_color_mode(), rgb)
}

pub fn theme() -> Theme {
    let color_mode = detected_color_mode();
    let no_color = matches!(color_mode, ColorMode::NoColor);

    Theme {
        accent: terminal_color(color_mode, DRACULA_CYAN),
        ok: terminal_color(color_mode, DRACULA_GREEN),
        warn: terminal_color(color_mode, DRACULA_YELLOW),
        err: terminal_color(color_mode, DRACULA_RED),
        dim: terminal_color(color_mode, DRACULA_COMMENT),
        cyan: terminal_color(color_mode, DRACULA_CYAN),
        surface: terminal_color(color_mode, DRACULA_SURFACE),
        no_color,
    }
}
