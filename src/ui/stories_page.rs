//! Stories / Status dedicated page: sidebar list and main viewer.

use egui::{
    Align, Align2, Color32, CornerRadius, Frame, Layout, Margin, Rect, Sense, Stroke,
    pos2, vec2,
};

use crate::app::App;
use crate::model::Action;
use crate::stories::ContactStories;
use crate::theme::{self, Icon, Palette};
use super::{stories, widgets};

/// Renders the status sidebar with "My status", "Recent updates", and "Viewed updates".
pub fn sidebar(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let panel = egui::Panel::left("stories_sidebar")
        .resizable(true)
        .default_size(app.settings.sidebar_width)
        .size_range(280.0..=520.0)
        .show_separator_line(false)
        .frame(Frame::new().fill(palette.panel).inner_margin(Margin::ZERO));

    let response = panel.show(ui, |ui| {
        // 1. Header
        header(app, ui, &palette);

        // 2. Status lists scroll area
        egui::ScrollArea::vertical()
            .id_salt("stories-sidebar-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);

                // My Status item
                my_status_row(app, ui, &palette);

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                let grouped = app.stories.grouped();
                let my_jid = app.me.clone().unwrap_or_default();

                // Partition into recent (unviewed) and viewed
                let (recent, viewed): (Vec<&ContactStories>, Vec<&ContactStories>) = grouped
                    .iter()
                    .filter(|c| c.sender != "me" && c.sender != my_jid && c.sender != "status@broadcast")
                    .partition(|c| c.has_unviewed);

                // Recent updates
                if !recent.is_empty() {
                    section_header(ui, &palette, &crate::i18n::gettext(app.locale, "Recent updates"));
                    for contact in &recent {
                        contact_status_row(app, ui, &palette, contact, true);
                    }
                    ui.add_space(6.0);
                }

                // Viewed updates
                if !viewed.is_empty() {
                    section_header(ui, &palette, &crate::i18n::gettext(app.locale, "Viewed updates"));
                    for contact in &viewed {
                        contact_status_row(app, ui, &palette, contact, false);
                    }
                }

                // If no contact updates
                if grouped.is_empty()
                    || (recent.is_empty() && viewed.is_empty())
                {
                    ui.add_space(24.0);
                    ui.vertical_centered(|ui| {
                        theme::paint_icon(
                            ui,
                            Icon::Status,
                            Rect::from_center_size(ui.cursor().center(), vec2(32.0, 32.0)),
                            24.0,
                            palette.dim,
                        );
                        ui.add_space(36.0);
                        theme::text(
                            ui,
                            crate::i18n::gettext(app.locale, "No recent status updates"),
                            theme::semibold(13.0),
                            palette.secondary,
                        );
                    });
                }
            });
    });

    let width = response.response.rect.width();
    if (width - app.settings.sidebar_width).abs() > 1.0 {
        app.settings.sidebar_width = width;
        app.actions.push(Action::SettingsChanged);
    }

    widgets::paint_edge_beside(ui, &palette, response.response.rect);
}

fn header(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    Frame::new()
        .inner_margin(Margin {
            left: 16,
            right: 14,
            top: 10,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                vec2(ui.available_width(), 36.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    theme::text(
                        ui,
                        crate::i18n::gettext(app.locale, "Status"),
                        theme::bold(20.0),
                        palette.text,
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::icon_button(
                            ui,
                            Icon::Plus,
                            18.0,
                            palette.secondary,
                            palette.text,
                            "Add status update",
                        )
                        .clicked()
                        {
                            app.actions.push(Action::OpenPostStory);
                        }
                    });
                },
            );
        });
}

fn section_header(ui: &mut egui::Ui, palette: &Palette, title: &str) {
    Frame::new()
        .inner_margin(Margin {
            left: 16,
            right: 16,
            top: 6,
            bottom: 6,
        })
        .show(ui, |ui| {
            theme::text(
                ui,
                title.to_uppercase(),
                theme::bold(11.0),
                palette.secondary,
            );
        });
}

fn my_status_row(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    let me_str = app.me.clone().unwrap_or_else(|| "me".to_string());
    let (has_stories, first_story_id, last_timestamp) = {
        let items = app
            .stories
            .stories_by_sender
            .get("me")
            .or_else(|| app.stories.stories_by_sender.get(&me_str))
            .filter(|items| !items.is_empty());
        match items {
            Some(items) => (
                true,
                items.first().map(|s| s.id.clone()),
                items.last().map(|s| s.timestamp),
            ),
            None => (false, None, None),
        }
    };

    let sender_for_viewer = if app.stories.stories_by_sender.contains_key(&me_str) {
        me_str.clone()
    } else {
        "me".to_string()
    };

    let is_selected = app.active_story_contact.as_deref() == Some(&sender_for_viewer)
        || app.active_story_contact.as_deref() == Some("me");

    let row_height = 64.0;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if response.clicked() {
        if has_stories {
            app.actions.push(Action::SelectStoryContact(sender_for_viewer.clone()));
        } else {
            app.actions.push(Action::OpenPostStory);
        }
    }

    if ui.is_rect_visible(rect) {
        if is_selected {
            ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.surface_hover);
        } else if response.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.surface);
        }

        // Avatar with ring or plus badge
        let avatar_center = pos2(rect.left() + 38.0, rect.center().y);
        let me = app.me.clone().unwrap_or_default();
        let me_name = app.me_name.clone().unwrap_or_else(|| "You".to_owned());
        let picture = app.avatar(&me);

        if has_stories {
            ui.painter().circle_stroke(
                avatar_center,
                24.0,
                Stroke::new(2.2, Color32::from_rgb(0, 168, 132)),
            );
        }

        ui.scope_builder(
            egui::UiBuilder::new().max_rect(Rect::from_center_size(avatar_center, vec2(42.0, 42.0))),
            |ui| {
                widgets::avatar(ui, palette, &me_name, &me, 42.0, picture.as_deref());
            },
        );

        if !has_stories {
            let badge_pos = pos2(avatar_center.x + 14.0, avatar_center.y + 14.0);
            ui.painter().circle_filled(badge_pos, 8.0, palette.panel);
            ui.painter().circle_filled(badge_pos, 7.0, Color32::from_rgb(0, 168, 132));
            ui.painter().text(
                badge_pos + vec2(0.0, -1.0),
                Align2::CENTER_CENTER,
                "+",
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
        }

        // Text labels
        let text_left = rect.left() + 72.0;
        let title_pos = pos2(text_left, rect.center().y - 10.0);
        let sub_pos = pos2(text_left, rect.center().y + 8.0);

        theme::paint_text(
            ui,
            title_pos,
            Align2::LEFT_CENTER,
            &crate::i18n::gettext(app.locale, "My status"),
            theme::semibold(14.0),
            palette.text,
        );

        let subtitle = if let Some(timestamp) = last_timestamp {
            stories::format_relative_time(timestamp)
        } else if has_stories {
            crate::i18n::gettext(app.locale, "Tap to view update").to_string()
        } else {
            crate::i18n::gettext(app.locale, "Tap to add status update").to_string()
        };

        theme::paint_text(
            ui,
            sub_pos,
            Align2::LEFT_CENTER,
            &subtitle,
            theme::regular(12.0),
            palette.secondary,
        );

        // Delete button if user has active stories
        if has_stories {
            let trash_rect = Rect::from_center_size(
                pos2(rect.right() - 28.0, rect.center().y),
                vec2(32.0, 32.0),
            );
            let trash_resp = ui.allocate_rect(trash_rect, Sense::click());
            if trash_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if trash_resp.clicked() {
                if let Some(ref id) = first_story_id {
                    app.actions.push(Action::RevokeStory(id.clone()));
                }
            }
            let tint = if trash_resp.hovered() { palette.danger } else { palette.secondary };
            theme::paint_icon(ui, Icon::Trash, trash_rect, 16.0, tint);
            trash_resp.on_hover_text("Delete status");
        }
    }
}

fn contact_status_row(
    app: &mut App,
    ui: &mut egui::Ui,
    palette: &Palette,
    contact: &ContactStories,
    is_recent: bool,
) {
    let is_selected = app.active_story_contact.as_deref() == Some(&contact.sender);
    let row_height = 64.0;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if response.clicked() {
        app.actions.push(Action::SelectStoryContact(contact.sender.clone()));
    }

    if ui.is_rect_visible(rect) {
        if is_selected {
            ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.surface_hover);
        } else if response.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.surface);
        }

        // Avatar with status ring
        let avatar_center = pos2(rect.left() + 38.0, rect.center().y);
        let ring_color = if is_recent {
            Color32::from_rgb(0, 168, 132) // WhatsApp green
        } else {
            palette.dim // Viewed gray
        };

        ui.painter().circle_stroke(
            avatar_center,
            24.0,
            Stroke::new(2.2, ring_color),
        );

        let picture = app.avatar(&contact.sender);
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(Rect::from_center_size(avatar_center, vec2(42.0, 42.0))),
            |ui| {
                widgets::avatar(
                    ui,
                    palette,
                    &contact.sender_name,
                    &contact.sender,
                    42.0,
                    picture.as_deref(),
                );
            },
        );

        // Name and timestamp
        let text_left = rect.left() + 72.0;
        let title_pos = pos2(text_left, rect.center().y - 10.0);
        let sub_pos = pos2(text_left, rect.center().y + 8.0);

        theme::paint_text(
            ui,
            title_pos,
            Align2::LEFT_CENTER,
            &contact.sender_name,
            theme::semibold(14.0),
            palette.text,
        );

        let time_str = stories::format_relative_time(contact.latest_timestamp);
        theme::paint_text(
            ui,
            sub_pos,
            Align2::LEFT_CENTER,
            &time_str,
            theme::regular(12.0),
            palette.secondary,
        );
    }
}

/// Central panel for Stories page: shows the active story viewer or an empty state placeholder.
pub fn show(app: &mut App, ui: &mut egui::Ui) {
    if app.story_viewer.is_some() {
        let rect = ui.available_rect_before_wrap();
        stories::viewer_show_in(app, ui.ctx(), Some(rect));
    } else {
        empty_placeholder(app, ui);
    }
}

fn empty_placeholder(app: &App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let rect = ui.available_rect_before_wrap();

    ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.panel);

    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.vertical_centered(|ui| {
            let center_y = rect.center().y - 60.0;
            let current_y = ui.cursor().top();
            let space = (center_y - current_y).max(40.0);
            ui.add_space(space);

            // Large circle with Status icon
            let icon_circle_size = 96.0;
            let (circle_rect, _) = ui.allocate_exact_size(vec2(icon_circle_size, icon_circle_size), Sense::hover());
            ui.painter().circle_filled(
                circle_rect.center(),
                icon_circle_size / 2.0,
                palette.surface_hover,
            );
            theme::paint_icon(
                ui,
                Icon::Status,
                circle_rect,
                48.0,
                palette.accent,
            );

            ui.add_space(20.0);

            theme::text(
                ui,
                crate::i18n::gettext(app.locale, "Click on a contact to view their status update"),
                theme::bold(18.0),
                palette.text,
            );

            ui.add_space(8.0);

            theme::text(
                ui,
                crate::i18n::gettext(app.locale, "Status updates disappear after 24 hours"),
                theme::regular(13.0),
                palette.secondary,
            );
        });
    });
}
