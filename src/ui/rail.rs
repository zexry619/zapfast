//! Left Navigation Rail: WhatsApp Web persistent icon column (width 64px).

use egui::{CornerRadius, Frame, Margin, Rect, Sense, pos2, vec2};

use super::focus::{Stop, TabStop};
use super::widgets;
use crate::app::App;
use crate::model::{Action, CallLogStatus, Page};
use crate::theme::{self, Icon, Palette};

pub const RAIL_WIDTH: f32 = 64.0;

enum BadgeKind {
    Dot(egui::Color32),
    Count(usize),
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let panel = egui::Panel::left("navigation_rail")
        .resizable(false)
        .exact_size(RAIL_WIDTH)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(palette.window)
                .inner_margin(Margin::symmetric(0, 8)),
        );

    let response = panel.show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let inset = theme::traffic_light_inset(ui.ctx());
        if inset > 0.0 {
            let (strip, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
            super::titlebar_drag(ui, strip);
        } else {
            ui.add_space(4.0);
        }

        let macos = theme::macos_chrome(ui.ctx());
        let bottom_height = if macos { 82.0 } else { 132.0 };

        // Top Navigation Icons
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            // 1. Chats
            let unread_chats = app.account().unread_chat_count();
            let is_chats = app.page == Page::Chats;
            let chats_badge = if unread_chats > 0 {
                Some(BadgeKind::Count(unread_chats as usize))
            } else {
                None
            };
            if nav_button(
                ui,
                &palette,
                Icon::MessageCircle,
                is_chats,
                chats_badge,
                &crate::i18n::gettext(app.locale, "Chats"),
            )
            .clicked()
            {
                app.actions.push(Action::Open(Page::Chats));
            }

            ui.add_space(6.0);

            // 2. Stories / Status
            let is_stories = app.page == Page::Stories;
            let my_jid = app.me.as_deref().unwrap_or_default();
            let has_unviewed =
                app.stories.grouped().iter().any(|c| {
                    c.sender != "status@broadcast" && c.sender != my_jid && c.has_unviewed
                });
            let status_badge = if has_unviewed {
                Some(BadgeKind::Dot(palette.accent))
            } else {
                None
            };
            if nav_button(
                ui,
                &palette,
                Icon::Status,
                is_stories,
                status_badge,
                &crate::i18n::gettext(app.locale, "Status"),
            )
            .clicked()
            {
                app.actions.push(Action::Open(Page::Stories));
            }

            ui.add_space(6.0);

            // 3. Calls
            let is_calls = app.page == Page::Calls;
            let unseen_missed = if is_calls {
                0
            } else {
                app.call_logs
                    .iter()
                    .filter(|c| {
                        c.status == CallLogStatus::Missed
                            && c.timestamp > app.settings.last_seen_call_timestamp
                    })
                    .count()
            };
            let calls_badge = if unseen_missed > 0 {
                Some(BadgeKind::Count(unseen_missed))
            } else {
                None
            };
            if nav_button(
                ui,
                &palette,
                Icon::Phone,
                is_calls,
                calls_badge,
                &crate::i18n::gettext(app.locale, "Calls"),
            )
            .clicked()
            {
                app.actions.push(Action::Open(Page::Calls));
            }
        });

        // Spacer pushing Starred, Settings & Profile avatar to bottom
        let space_to_bottom = (ui.available_height() - bottom_height).max(0.0);
        ui.add_space(space_to_bottom);

        // Bottom Navigation Icons
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            // Starred messages
            let is_starred_open = matches!(
                app.dialog,
                Some(crate::model::Dialog::StarredMessages { .. })
            );
            if nav_button(
                ui,
                &palette,
                Icon::Star,
                is_starred_open,
                None,
                &crate::i18n::gettext(app.locale, "Starred messages"),
            )
            .clicked()
            {
                app.actions
                    .push(Action::ShowDialog(crate::model::Dialog::StarredMessages {
                        chat: None,
                    }));
            }

            ui.add_space(6.0);

            // Settings
            if !macos {
                let is_settings = app.page == Page::Settings;
                let settings_label = if is_settings {
                    crate::i18n::gettext(app.locale, "Close settings (Ctrl+,)")
                } else {
                    crate::i18n::gettext(app.locale, "Settings (Ctrl+,)")
                };
                if nav_button(
                    ui,
                    &palette,
                    Icon::Settings,
                    is_settings,
                    None,
                    &settings_label,
                )
                .tab_stop(Stop::Settings)
                .clicked()
                {
                    app.actions.push(Action::ToggleSettings);
                }

                ui.add_space(6.0);
            }

            // Profile Avatar
            super::accounts::avatar_button(app, ui, 32.0).tab_stop(Stop::Profile);
        });
    });

    widgets::paint_edge_beside(ui, &palette, response.response.rect);
}

fn nav_button(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: Icon,
    is_active: bool,
    badge: Option<BadgeKind>,
    tooltip: &str,
) -> egui::Response {
    let size = vec2(44.0, 44.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    theme::reveal_focus(&response);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), tooltip)
    });
    let hovered = response.hovered();

    if ui.is_rect_visible(rect) {
        // Draw background pill
        if is_active {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(12), palette.surface_active);
        } else if hovered {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(12), palette.surface_hover);
        }

        // Draw icon
        let icon_color = if is_active {
            palette.accent
        } else if hovered {
            palette.text
        } else {
            palette.secondary
        };

        let icon_size = 22.0;
        let scale = if response.is_pointer_button_down_on() {
            0.92
        } else {
            1.0
        };
        theme::paint_icon(ui, icon, rect, icon_size * scale, icon_color);

        // Draw badge if any
        match badge {
            Some(BadgeKind::Dot(color)) => {
                let dot_pos = pos2(rect.right() - 8.0, rect.top() + 8.0);
                ui.painter().circle_filled(dot_pos, 5.0, palette.window);
                ui.painter().circle_filled(dot_pos, 3.5, color);
            }
            Some(BadgeKind::Count(count)) if count > 0 => {
                let label = if count > 99 {
                    "99+".to_string()
                } else {
                    count.to_string()
                };
                let galley =
                    ui.painter()
                        .layout_no_wrap(label, theme::bold(10.0), palette.on_accent);
                let badge_h = 16.0;
                let badge_w = (galley.size().x + 8.0).max(badge_h);
                let badge_center = pos2(rect.right() - 6.0, rect.top() + 8.0);
                let badge_rect = Rect::from_center_size(badge_center, vec2(badge_w, badge_h));

                ui.painter().rect_filled(
                    badge_rect.expand(1.5),
                    CornerRadius::same(10),
                    palette.window,
                );
                ui.painter()
                    .rect_filled(badge_rect, CornerRadius::same(8), palette.accent);
                ui.painter().galley(
                    badge_rect.center() - galley.size() / 2.0,
                    galley,
                    palette.on_accent,
                );
            }
            _ => {}
        }
    }

    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if !tooltip.is_empty() {
        response.on_hover_text(tooltip)
    } else {
        response
    }
}
