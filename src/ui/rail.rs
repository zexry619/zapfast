//! Left Navigation Rail: WhatsApp Web 3-zone persistent icon column (width ~54px).

use egui::{CornerRadius, Frame, Margin, Rect, Sense, pos2, vec2};

use crate::app::App;
use crate::model::{Action, CallLogStatus, Page};
use crate::theme::{self, Icon, Palette};
use super::widgets;

pub const RAIL_WIDTH: f32 = 54.0;

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
        .frame(Frame::new().fill(palette.panel).inner_margin(Margin::symmetric(4, 6)));

    let response = panel.show(ui, |ui| {
        let inset = theme::traffic_light_inset(ui.ctx());
        if inset > 0.0 {
            let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
            super::titlebar_drag(ui, strip);
        } else {
            ui.add_space(4.0);
        }

        let bottom_height = 92.0;

        // Top Navigation Icons
        ui.vertical_centered(|ui| {
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
            ) {
                app.actions.push(Action::Open(Page::Chats));
            }

            ui.add_space(6.0);

            // 2. Stories / Status
            let is_stories = app.page == Page::Stories;
            let my_jid = app.me.as_deref().unwrap_or_default();
            let has_unviewed = app.stories.grouped().iter().any(|c| {
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
            ) {
                app.actions.push(Action::Open(Page::Stories));
            }

            ui.add_space(6.0);

            // 3. Calls
            let is_calls = app.page == Page::Calls;
            let missed_calls = app
                .call_logs
                .iter()
                .filter(|c| c.status == CallLogStatus::Missed)
                .count();
            let calls_badge = if missed_calls > 0 {
                Some(BadgeKind::Count(missed_calls))
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
            ) {
                app.actions.push(Action::Open(Page::Calls));
            }
        });

        // Spacer pushing Settings & Profile avatar to bottom
        let space_to_bottom = (ui.available_height() - bottom_height).max(0.0);
        ui.add_space(space_to_bottom);

        // Bottom Navigation Icons
        ui.vertical_centered(|ui| {
            // Settings
            let is_settings = app.page == Page::Settings;
            if nav_button(
                ui,
                &palette,
                Icon::Settings,
                is_settings,
                None,
                &crate::i18n::gettext(app.locale, "Settings"),
            ) {
                app.actions.push(Action::ToggleSettings);
            }

            ui.add_space(10.0);

            // Profile Avatar
            super::accounts::avatar_button(app, ui, 34.0);
            ui.add_space(6.0);
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
) -> bool {
    let size = vec2(42.0, 42.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();

    if ui.is_rect_visible(rect) {
        // Draw background pill
        if is_active {
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(10),
                palette.surface_hover,
            );
            // Left active indicator pill
            let indicator_rect = Rect::from_min_size(
                pos2(rect.left() - 3.0, rect.top() + 8.0),
                vec2(3.0, rect.height() - 16.0),
            );
            ui.painter().rect_filled(indicator_rect, CornerRadius::same(2), palette.accent);
        } else if hovered {
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(10),
                palette.surface_hover,
            );
        }

        // Draw icon
        let icon_color = if is_active {
            palette.accent
        } else if hovered {
            palette.text
        } else {
            palette.secondary
        };

        let icon_size = 20.0;
        let scale = if response.is_pointer_button_down_on() { 0.92 } else { 1.0 };
        theme::paint_icon(ui, icon, rect, icon_size * scale, icon_color);

        // Draw badge if any
        match badge {
            Some(BadgeKind::Dot(color)) => {
                let dot_pos = pos2(rect.right() - 8.0, rect.top() + 8.0);
                ui.painter().circle_filled(dot_pos, 4.0, color);
            }
            Some(BadgeKind::Count(count)) if count > 0 => {
                let badge_pos = pos2(rect.right() - 6.0, rect.top() + 6.0);
                widgets::badge(ui, palette, badge_pos, count as u32, false);
            }
            _ => {}
        }
    }

    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let response = if !tooltip.is_empty() {
        response.on_hover_text(tooltip)
    } else {
        response
    };

    response.clicked()
}
