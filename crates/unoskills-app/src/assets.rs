use std::borrow::Cow;

use gpui_kit::component::IconNamed;
use gpui_kit::{AssetSource, Result, SharedString};

gpui_kit::assets::icon_assets!(
    AdditionalIcons,
    [
        ArchiveRestore,
        BadgeCheck,
        Cloud,
        Download,
        GitFork,
        House,
        Layers,
        Monitor,
        SlidersHorizontal,
        Store,
        Trophy,
        Users,
    ]
);

const UNO_MARK_SVG: &[u8] = include_bytes!("../assets/icons/uno-mark.svg");
const UNO_APP_ICON_SVG: &[u8] = include_bytes!("../assets/icons/uno-app-icon.svg");

/// Typed monochrome brand icons for UnoSkills, usable directly with `Icon::new(AppIcon::UnoMark)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppIcon {
    UnoMark,
}

impl IconNamed for AppIcon {
    fn path(self) -> SharedString {
        match self {
            Self::UnoMark => "icons/uno-mark.svg".into(),
        }
    }
}

/// Composite `AssetSource` following https://gpui-kit.com/docs/assets/:
/// Serves app branding, additional Lucide icons and the default component bundle.
pub struct UnoAssets;

impl AssetSource for UnoAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match path {
            "icons/uno-mark.svg" => Ok(Some(Cow::Borrowed(UNO_MARK_SVG))),
            "icons/uno-app-icon.svg" => Ok(Some(Cow::Borrowed(UNO_APP_ICON_SVG))),
            _ => match AdditionalIcons.load(path)? {
                Some(icon) => Ok(Some(icon)),
                None => gpui_kit::assets::Assets.load(path),
            },
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut items = gpui_kit::assets::Assets.list(path)?;
        items.extend(AdditionalIcons.list(path)?);
        for custom in ["icons/uno-mark.svg", "icons/uno-app-icon.svg"] {
            if custom.starts_with(path) {
                items.push(SharedString::from(custom));
            }
        }
        Ok(items)
    }
}
