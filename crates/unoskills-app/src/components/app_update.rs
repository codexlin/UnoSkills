use gpui_kit::component::{
    ActiveTheme, Disableable, IconName, Sizable, StyledExt, WindowExt,
    alert::Alert,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
    h_flex,
    progress::Progress,
    v_flex,
};
use gpui_kit::{App, Entity, IntoElement, ParentElement, Styled, Window, div, px};
use unoskills_core::updater::UpdateProgress;

use crate::i18n::Strings;
use crate::state::{UpdateDownload, WorkspaceState, platform_install_button_label};

pub fn status_label(state: &UpdateDownload, s: &Strings) -> Option<String> {
    Some(match state {
        UpdateDownload::Idle => return None,
        UpdateDownload::InProgress(UpdateProgress::Preparing) => s.update_preparing.into(),
        UpdateDownload::InProgress(UpdateProgress::Downloading(Some(percent))) => {
            format!("{} {percent}%", s.update_downloading)
        }
        UpdateDownload::InProgress(UpdateProgress::Downloading(None)) => {
            s.update_downloading.into()
        }
        UpdateDownload::InProgress(UpdateProgress::Verifying) => s.update_verifying.into(),
        UpdateDownload::Ready(_) => s.update_ready.into(),
        UpdateDownload::Failed(_) => s.update_failed.into(),
    })
}

pub fn open_dialog(workspace: Entity<WorkspaceState>, window: &mut Window, cx: &mut App) {
    if window.has_active_dialog(cx) {
        return;
    }
    window.open_dialog(cx, move |dialog, _, cx| {
        let state = workspace.read(cx);
        let s = state.t();
        let dialog = dialog.title(s.settings_updates).centered().width(px(420.));
        let Some((outcome, release)) = state
            .available_update()
            .and_then(|outcome| outcome.latest.as_ref().map(|release| (outcome, release)))
        else {
            return dialog;
        };
        let asset = outcome.recommended_asset.as_ref();
        let download = state.update_download();
        let status = status_label(download, s);
        let muted = cx.theme().muted_foreground;
        let can_download = asset.is_some() && !state.checking_updates();
        let release_page = {
            let url = release.html_url.clone();
            Button::new("update-dialog-release")
                .ghost()
                .label(s.btn_open_release)
                .disabled(url.is_empty())
                .on_click(move |_, _, cx| cx.open_url(&url))
        };
        let download_button = |label| {
            let ws = workspace.clone();
            Button::new("update-dialog-download")
                .primary()
                .icon(IconName::ArrowDown)
                .label(label)
                .disabled(!can_download)
                .on_click(move |_, window, cx| {
                    ws.update(cx, |ws, cx| ws.download_update_async(window, cx));
                })
        };
        let dismiss = |label| {
            Button::new("update-dialog-dismiss")
                .outline()
                .label(label)
                .on_click(|_, window, cx| window.close_dialog(cx))
        };
        let footer = match download {
            UpdateDownload::Idle if asset.is_some() => DialogFooter::new()
                .child(dismiss(s.btn_cancel))
                .child(download_button(if outcome.checksums_asset.is_some() {
                    s.btn_download_update
                } else {
                    s.update_start_download
                })),
            UpdateDownload::Idle => DialogFooter::new().child(release_page),
            UpdateDownload::InProgress(_) => {
                DialogFooter::new().child(dismiss(s.update_background))
            }
            UpdateDownload::Ready(_) => {
                let ws = workspace.clone();
                DialogFooter::new().child(dismiss(s.btn_close)).child(
                    Button::new("update-dialog-install")
                        .primary()
                        .label(platform_install_button_label(s))
                        .on_click(move |_, window, cx| {
                            ws.update(cx, |ws, cx| ws.apply_downloaded_update(window, cx));
                            window.close_dialog(cx);
                        }),
                )
            }
            UpdateDownload::Failed(_) => DialogFooter::new()
                .child(release_page)
                .child(download_button(s.update_retry)),
        };

        let summary = h_flex()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_lg()
                    .font_semibold()
                    .child(release.tag_name.clone()),
            )
            .children(asset.filter(|asset| asset.size > 0).map(|asset| {
                div()
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(muted)
                    .child(format!("{:.1} MiB", asset.size as f64 / 1_048_576.0))
            }));

        let body = match download {
            UpdateDownload::Idle => div()
                .text_sm()
                .text_color(muted)
                .child(if asset.is_none() {
                    s.update_release_only
                } else if outcome.checksums_asset.is_some() {
                    s.update_confirm_hint
                } else {
                    s.update_unverified_hint
                })
                .into_any_element(),
            UpdateDownload::InProgress(progress) => {
                let label = status.unwrap_or_default();
                let percent = match progress {
                    UpdateProgress::Downloading(percent) => *percent,
                    _ => None,
                };
                v_flex()
                    .gap_2()
                    .child(div().text_sm().child(gpui_kit::text!(label.clone())))
                    .child(
                        Progress::new("app-update-progress")
                            .small()
                            .loading(percent.is_none())
                            .value(f32::from(percent.unwrap_or_default()))
                            .accessibility_label(label),
                    )
                    .into_any_element()
            }
            UpdateDownload::Ready(downloaded) => div()
                .text_sm()
                .text_color(muted)
                .child(gpui_kit::text!(if downloaded.verified {
                    s.badge_sha256_verified
                } else {
                    s.update_unverified_hint
                }))
                .into_any_element(),
            UpdateDownload::Failed(error) => Alert::error(
                "update-download-error",
                format!("{}\n{error}", s.update_download_failed),
            )
            .small()
            .into_any_element(),
        };
        let content = v_flex().w_full().gap_3().child(summary).child(body);
        dialog.child(content).footer(footer)
    });
}
