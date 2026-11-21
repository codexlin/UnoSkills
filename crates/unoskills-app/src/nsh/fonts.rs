use nsh_core::prelude::*;

const PRESET_FONTS: &[&[u8]] = &[
    include_bytes!("../../assets/fonts/Oxanium.ttf"),
    include_bytes!("../../assets/fonts/FiraCode.ttf"),
    include_bytes!("../../assets/fonts/Geist.ttf"),
    include_bytes!("../../assets/fonts/GeistMono.ttf"),
    include_bytes!("../../assets/fonts/Outfit.ttf"),
    include_bytes!("../../assets/fonts/Montserrat.ttf"),
    include_bytes!("../../assets/fonts/DMSans.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-SemiBold.ttf"),
    include_bytes!("../../assets/fonts/IBMPlexMono-Bold.ttf"),
];

#[module]
pub(super) struct Fonts;

#[module]
impl Fonts {
    /// Register the packaged theme fonts for this application surface.
    pub async fn load() {
        for font in PRESET_FONTS {
            if let Err(error) = register_font(font.to_vec()).await {
                eprintln!("UnoSkills: could not register a theme font: {error}");
            }
        }
    }

    /// Make the notices readable in a standalone desktop build.
    pub fn licenses() -> String {
        [
            include_str!("../../assets/fonts/oxanium-OFL.txt"),
            include_str!("../../assets/fonts/firacode-OFL.txt"),
            include_str!("../../assets/fonts/geist-OFL.txt"),
            include_str!("../../assets/fonts/outfit-OFL.txt"),
            include_str!("../../assets/fonts/montserrat-OFL.txt"),
            include_str!("../../assets/fonts/dmsans-OFL.txt"),
            include_str!("../../assets/fonts/ibmplexmono-OFL.txt"),
        ]
        .join("\n\n")
    }
}
