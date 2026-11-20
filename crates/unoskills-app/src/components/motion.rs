use gpui_kit::App;
use gpui_kit::base::Transition;
use gpui_kit::component::ActiveTheme;

pub fn control_transition(cx: &App) -> Transition {
    let tokens = cx.theme().motion_tokens();
    Transition::new(tokens.duration_fast).easing(tokens.easing_move.clone())
}
