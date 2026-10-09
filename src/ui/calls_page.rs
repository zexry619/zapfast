//! Calls dedicated page: sidebar call history list and main detail pane.

use egui::{
    Align, Align2, Color32, CornerRadius, Frame, Layout, Margin, Rect, Sense, Vec2,
    pos2, vec2,
};

use crate::app::App;
use crate::model::{Action, CallLogEntry, CallLogStatus};
use crate::theme::{self, Icon, Palette};
use super::widgets;

pub fn sidebar(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let panel = egui::Panel::left("calls_sidebar")
        .resizable(true)
        .default_size(app.settings.sidebar_width)
        .size_range(280.0..=520.0)
        .show_separator_line(false)
        .frame(Frame::new().fill(palette.panel).inner_margin(Margin::ZERO));

    let response = panel.show(ui, |ui| {
        // 1. Header
        header(app, ui, &palette);

        // 2. Filter chips row (All / Missed)
        filter_chips(app, ui, &palette);

        // 3. Call history list
        egui::ScrollArea::vertical()
            .id_salt("calls-sidebar-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);

                let filtered_logs: Vec<CallLogEntry> = app
                    .call_logs
                    .iter()
                    .filter(|entry| {
                        if app.calls_filter_missed {
                            entry.status == CallLogStatus::Missed
                        } else {
                            true
                        }
                    })
                    .cloned()
                    .collect();

                if filtered_logs.is_empty() {
                    ui.add_space(32.0);
                    ui.vertical_centered(|ui| {
                        theme::paint_icon(
                            ui,
                            Icon::Phone,
                            Rect::from_center_size(ui.cursor().center(), vec2(32.0, 32.0)),
                            24.0,
                            palette.dim,
                        );
                        ui.add_space(36.0);
                        theme::text(
                            ui,
                            if app.calls_filter_missed {
                                crate::i18n::gettext(app.locale, "No missed calls")
                            } else {
                                crate::i18n::gettext(app.locale, "No call history yet")
                            },
                            theme::semibold(13.0),
                            palette.secondary,
                        );
                    });
                } else {
                    for entry in &filtered_logs {
                        call_log_row(app, ui, &palette, entry);
                    }
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
            bottom: 6,
        })
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                vec2(ui.available_width(), 36.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    theme::text(
                        ui,
                        crate::i18n::gettext(app.locale, "Calls"),
                        theme::bold(20.0),
                        palette.text,
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::icon_button(
                            ui,
                            Icon::Refresh,
                            18.0,
                            palette.secondary,
                            palette.text,
                            "Refresh call history",
                        )
                        .clicked()
                        {
                            app.actions.push(Action::FetchCallLogs);
                        }
                    });
                },
            );
        });
}

fn filter_chips(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    Frame::new()
        .inner_margin(Margin {
            left: 14,
            right: 14,
            top: 4,
            bottom: 8,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 0.0);

                let all_selected = !app.calls_filter_missed;
                if chip_button(ui, palette, &crate::i18n::gettext(app.locale, "All"), all_selected) {
                    if app.calls_filter_missed {
                        app.actions.push(Action::ToggleCallsFilterMissed);
                    }
                }

                let missed_count = app
                    .call_logs
                    .iter()
                    .filter(|c| c.status == CallLogStatus::Missed)
                    .count();
                let missed_label = if missed_count > 0 {
                    format!("{} ({})", crate::i18n::gettext(app.locale, "Missed"), missed_count)
                } else {
                    crate::i18n::gettext(app.locale, "Missed").to_string()
                };

                if chip_button(ui, palette, &missed_label, app.calls_filter_missed) {
                    if !app.calls_filter_missed {
                        app.actions.push(Action::ToggleCallsFilterMissed);
                    }
                }
            });
        });
}

fn chip_button(ui: &mut egui::Ui, palette: &Palette, label: &str, selected: bool) -> bool {
    let padding = vec2(12.0, 6.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::semibold(12.0), palette.text);
    let size = galley.size() + padding * 2.0;

    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let (bg, text_color) = if selected {
            (palette.accent.gamma_multiply(0.2), palette.accent)
        } else if response.hovered() {
            (palette.surface_hover, palette.text)
        } else {
            (palette.surface, palette.secondary)
        };

        ui.painter().rect_filled(rect, CornerRadius::same(14), bg);
        ui.painter().galley(
            rect.center() - galley.size() / 2.0,
            galley,
            text_color,
        );
    }

    response.clicked()
}

fn call_log_row(app: &mut App, ui: &mut egui::Ui, palette: &Palette, entry: &CallLogEntry) {
    let is_selected = app.active_call_log.as_deref() == Some(&entry.call_id);
    let row_height = 64.0;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if response.clicked() {
        app.actions.push(Action::SelectCallLog(entry.call_id.clone()));
    }

    if ui.is_rect_visible(rect) {
        if is_selected {
            ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.surface_hover);
        } else if response.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.surface);
        }

        // Contact Avatar
        let avatar_center = pos2(rect.left() + 36.0, rect.center().y);
        let name = entry.peer_name.as_deref().unwrap_or(&entry.peer);
        let picture = app.avatar(&entry.peer);

        ui.scope_builder(
            egui::UiBuilder::new().max_rect(Rect::from_center_size(avatar_center, vec2(40.0, 40.0))),
            |ui| {
                widgets::avatar(ui, palette, name, &entry.peer, 40.0, picture.as_deref());
            },
        );

        // Name and details
        let text_left = rect.left() + 68.0;
        let title_pos = pos2(text_left, rect.center().y - 10.0);
        let sub_pos = pos2(text_left, rect.center().y + 8.0);

        let name_color = if entry.status == CallLogStatus::Missed {
            palette.danger
        } else {
            palette.text
        };

        theme::paint_text(
            ui,
            title_pos,
            Align2::LEFT_CENTER,
            name,
            theme::semibold(14.0),
            name_color,
        );

        // Status arrow and details
        let (arrow, arrow_color, status_text) = match entry.status {
            CallLogStatus::Missed => ("↙", palette.danger, "Missed"),
            _ if entry.from_me => ("↗", palette.accent, "Outgoing"),
            _ => ("↙", palette.accent, "Incoming"),
        };

        let stamp = crate::util::chat_stamp(app.locale, entry.timestamp);
        let duration_str = format_duration(entry.duration);
        let detail_str = if duration_str.is_empty() {
            format!("{arrow} {status_text} • {stamp}")
        } else {
            format!("{arrow} {status_text} • {stamp} ({duration_str})")
        };

        theme::paint_text(
            ui,
            sub_pos,
            Align2::LEFT_CENTER,
            &detail_str,
            theme::regular(12.0),
            if entry.status == CallLogStatus::Missed { arrow_color } else { palette.secondary },
        );

        // Direct call buttons on right
        let right_x = rect.right() - 24.0;
        let phone_rect = Rect::from_center_size(pos2(right_x, rect.center().y), vec2(30.0, 30.0));
        let phone_resp = ui.allocate_rect(phone_rect, Sense::click());
        if phone_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if phone_resp.clicked() {
            if entry.is_video {
                app.actions.push(Action::StartVideoCall(entry.peer.clone()));
            } else {
                app.actions.push(Action::StartCall(entry.peer.clone()));
            }
        }

        let icon = if entry.is_video { Icon::Video } else { Icon::Phone };
        let icon_color = if phone_resp.hovered() { palette.accent } else { palette.secondary };
        theme::paint_icon(ui, icon, phone_rect, 16.0, icon_color);
        phone_resp.on_hover_text(if entry.is_video { "Start video call" } else { "Start voice call" });
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let selected_entry = app
        .active_call_log
        .as_ref()
        .and_then(|id| app.call_logs.iter().find(|c| &c.call_id == id))
        .cloned();

    match selected_entry {
        Some(entry) => detail_panel(app, ui, &entry),
        None => empty_placeholder(app, ui),
    }
}

fn detail_panel(app: &mut App, ui: &mut egui::Ui, entry: &CallLogEntry) {
    let palette = app.palette;
    let rect = ui.available_rect_before_wrap();

    ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.panel);

    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        egui::ScrollArea::vertical()
            .id_salt("call-detail-scroll")
            .show(ui, |ui| {
                ui.add_space(40.0);

                // Big Avatar
                let name = entry.peer_name.as_deref().unwrap_or(&entry.peer);
                let picture = app.avatar(&entry.peer);
                ui.vertical_centered(|ui| {
                    let avatar_size = 80.0;
                    let (avatar_rect, _) = ui.allocate_exact_size(Vec2::splat(avatar_size), Sense::hover());
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(avatar_rect),
                        |ui| {
                            widgets::avatar(ui, &palette, name, &entry.peer, avatar_size, picture.as_deref());
                        },
                    );

                    ui.add_space(14.0);

                    // Name
                    theme::text(ui, name, theme::bold(22.0), palette.text);
                    ui.add_space(4.0);

                    // Peer / phone number
                    theme::text(ui, &entry.peer, theme::regular(14.0), palette.secondary);

                    ui.add_space(20.0);

                    // Call Action Buttons
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(12.0, 0.0);
                        let total_w = 130.0 * 3.0 + 24.0;
                        let pad = (ui.available_width() - total_w).max(0.0) / 2.0;
                        ui.add_space(pad);

                        // Voice Call Button
                        if action_pill_button(ui, &palette, Icon::Phone, "Voice Call", palette.accent) {
                            app.actions.push(Action::StartCall(entry.peer.clone()));
                        }

                        // Video Call Button
                        if action_pill_button(ui, &palette, Icon::Video, "Video Call", palette.accent) {
                            app.actions.push(Action::StartVideoCall(entry.peer.clone()));
                        }

                        // Message Button
                        if action_pill_button(ui, &palette, Icon::MessageCircle, "Message", palette.secondary) {
                            app.actions.push(Action::OpenChat(entry.peer.clone()));
                        }
                    });
                });

                ui.add_space(32.0);
                ui.separator();
                ui.add_space(16.0);

                // History with this contact
                Frame::new()
                    .inner_margin(Margin::symmetric(24, 0))
                    .show(ui, |ui| {
                        theme::text(
                            ui,
                            crate::i18n::gettext(app.locale, "CALL HISTORY WITH THIS CONTACT"),
                            theme::bold(12.0),
                            palette.secondary,
                        );

                        ui.add_space(10.0);

                        let related_logs: Vec<&CallLogEntry> = app
                            .call_logs
                            .iter()
                            .filter(|c| c.peer == entry.peer)
                            .collect();

                        for item in related_logs {
                            let (arrow, arrow_color, status_text) = match item.status {
                                CallLogStatus::Missed => ("↙", palette.danger, "Missed"),
                                _ if item.from_me => ("↗", palette.accent, "Outgoing"),
                                _ => ("↙", palette.accent, "Incoming"),
                            };

                            let stamp = crate::util::chat_stamp(app.locale, item.timestamp);
                            let dur = format_duration(item.duration);
                            let media_type = if item.is_video { "Video Call" } else { "Voice Call" };

                            let (row_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
                            ui.painter().rect_filled(row_rect, CornerRadius::same(6), palette.surface);

                            let left_x = row_rect.left() + 14.0;
                            let title = format!("{arrow} {status_text} {media_type}");
                            theme::paint_text(
                                ui,
                                pos2(left_x, row_rect.center().y),
                                Align2::LEFT_CENTER,
                                &title,
                                theme::semibold(13.0),
                                arrow_color,
                            );

                            let right_text = if dur.is_empty() { stamp } else { format!("{stamp} ({dur})") };
                            theme::paint_text(
                                ui,
                                pos2(row_rect.right() - 14.0, row_rect.center().y),
                                Align2::RIGHT_CENTER,
                                &right_text,
                                theme::regular(12.0),
                                palette.secondary,
                            );

                            ui.add_space(4.0);
                        }
                    });
            });
    });
}

fn action_pill_button(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: Icon,
    label: &str,
    accent: Color32,
) -> bool {
    let size = vec2(130.0, 38.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let bg = if response.hovered() {
            palette.surface_hover
        } else {
            palette.surface
        };

        ui.painter().rect_filled(rect, CornerRadius::same(19), bg);
        let icon_rect = Rect::from_center_size(pos2(rect.left() + 24.0, rect.center().y), vec2(18.0, 18.0));
        theme::paint_icon(ui, icon, icon_rect, 16.0, accent);

        theme::paint_text(
            ui,
            pos2(rect.left() + 42.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            theme::semibold(13.0),
            palette.text,
        );
    }

    response.clicked()
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

            let icon_circle_size = 96.0;
            let (circle_rect, _) = ui.allocate_exact_size(vec2(icon_circle_size, icon_circle_size), Sense::hover());
            ui.painter().circle_filled(
                circle_rect.center(),
                icon_circle_size / 2.0,
                palette.surface_hover,
            );
            theme::paint_icon(
                ui,
                Icon::Phone,
                circle_rect,
                48.0,
                palette.accent,
            );

            ui.add_space(20.0);

            theme::text(
                ui,
                crate::i18n::gettext(app.locale, "End-to-end encrypted calls"),
                theme::bold(18.0),
                palette.text,
            );

            ui.add_space(8.0);

            theme::text(
                ui,
                crate::i18n::gettext(app.locale, "Make private voice and video calls with ZapFast"),
                theme::regular(13.0),
                palette.secondary,
            );
        });
    });
}

fn format_duration(seconds: i64) -> String {
    if seconds <= 0 {
        return String::new();
    }
    let m = seconds / 60;
    let s = seconds % 60;
    if m == 0 {
        format!("{s}s")
    } else {
        format!("{m}m {s}s")
    }
}
