use egui::{FontData, FontDefinitions, FontFamily};

const INTER_VARIABLE: &[u8] = include_bytes!("../assets/fonts/InterVariable.ttf");
const JETBRAINS_MONO_VARIABLE: &[u8] = include_bytes!("../assets/fonts/JetBrainsMonoVariable.ttf");

pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "inter".to_owned(),
        std::sync::Arc::new(FontData::from_static(INTER_VARIABLE)),
    );
    fonts.font_data.insert(
        "jetbrains-mono".to_owned(),
        std::sync::Arc::new(FontData::from_static(JETBRAINS_MONO_VARIABLE)),
    );

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".to_owned());

    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "jetbrains-mono".to_owned());

    fonts
}
