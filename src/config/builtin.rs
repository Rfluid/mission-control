//! The built-in layer, compiled in from `defaults/`.

pub const CONFIG: &str = include_str!("../../defaults/config.toml");

pub const TEMPLATES: [(&str, &str); 4] = [
    ("minimal", include_str!("../../defaults/templates/minimal.toml")),
    ("multi-agent-build", include_str!("../../defaults/templates/multi-agent-build.toml")),
    ("pr-babysit", include_str!("../../defaults/templates/pr-babysit.toml")),
    ("research", include_str!("../../defaults/templates/research.toml")),
];
