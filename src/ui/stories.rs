//! WhatsApp Stories / Status UI: reel bar, full-screen viewer, and status composer.

use std::time::{Duration, Instant};

use egui::{
    Align, Align2, Color32, CornerRadius, Frame, Layout, Margin, Rect, Sense, Stroke, Vec2,
    pos2, vec2,
};

use crate::app::App;
use crate::model::Action;
use crate::stories::ContactStories;
use crate::theme::{self, Icon, Palette};
use crate::ui::widgets;

pub const STORY_COLORS: &[(u32, &str)] = &[
    (0xFF00A884, "Teal"),
    (0xFF128C7E, "Green"),
    (0xFF6B52AE, "Purple"),
    (0xFFB8336A, "Magenta"),
    (0xFF203554, "Navy"),
    (0xFFD85140, "Coral"),
    (0xFFE08B1B, "Amber"),
    (0xFF333D42, "Charcoal"),
];

pub const STORY_FONTS: &[(&str, u32)] = &[
    ("System", 0),
    ("Script", 2),
    ("Bold", 6),
    ("Serif", 8),
];

/// State of the active story viewer overlay.
#[derive(Clone, Debug)]
pub struct StoryViewerState {
    pub sender: String,
    pub index: usize,
    pub started_at: Instant,
    pub paused: bool,
}

impl StoryViewerState {
    pub fn new(sender: String, index: usize) -> Self {
        Self {
            sender,
            index,
            started_at: Instant::now(),
            paused: false,
        }
    }
}

/// Formats a unix timestamp in seconds to a human-readable relative time.
fn format_relative_time(timestamp: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(timestamp);

    let diff = now.saturating_sub(timestamp);
    if diff < 60 {
        "Just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        "Yesterday".to_string()
    }
}

/// Converts an ARGB u32 (0xAARRGGBB) to egui Color32.
fn argb_to_color32(argb: u32) -> Color32 {
    let r = ((argb >> 16) & 0xFF) as u8;
    let g = ((argb >> 8) & 0xFF) as u8;
    let b = (argb & 0xFF) as u8;
    Color32::from_rgb(r, g, b)
}

/// Draws the horizontal status / story reel above the chat list in the sidebar.
pub fn bar(app: &mut App, ui: &mut egui::Ui) {
    if !app.search.trim().is_empty() || app.locked_folder || app.show_archived {
        return;
    }

    let palette = app.palette;
    let grouped = app.stories.grouped();

    Frame::new()
        .inner_margin(Margin {
            left: 14,
            right: 14,
            top: 2,
            bottom: 8,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                theme::text(
                    ui,
                    crate::i18n::gettext(app.locale, "Status"),
                    theme::bold(13.0),
                    palette.secondary,
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if theme::icon_button(
                        ui,
                        Icon::Plus,
                        14.0,
                        palette.secondary,
                        palette.text,
                        "Add Status",
                    )
                    .clicked()
                    {
                        app.actions.push(Action::OpenPostStory);
                    }
                });
            });

            ui.add_space(4.0);

            egui::ScrollArea::horizontal()
                .id_salt("stories-reel")
                .animated(false)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(10.0, 0.0);

                        // 1. "My Status" card
                        my_status_item(app, ui, &palette);

                        // 2. Contacts with stories
                        for contact in &grouped {
                            if contact.sender == "me" || contact.sender == app.me.as_deref().unwrap_or("") {
                                continue;
                            }
                            contact_story_item(app, ui, &palette, contact);
                        }
                    });
                });

            ui.add_space(6.0);
            ui.separator();
        });
}

fn fit_inside(area: Rect, size: Vec2) -> Rect {
    if size.x <= 0.0 || size.y <= 0.0 {
        return area;
    }
    let scale = (area.width() / size.x).min(area.height() / size.y).min(1.0);
    let fitted = size * scale;
    Rect::from_center_size(area.center(), fitted)
}

fn my_status_item(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    let item_size = vec2(54.0, 68.0);
    let (rect, response) = ui.allocate_exact_size(item_size, Sense::click());

    let me_str = app.me.clone().unwrap_or_else(|| "me".to_string());
    let my_stories = app
        .stories
        .stories_by_sender
        .get("me")
        .or_else(|| app.stories.stories_by_sender.get(&me_str))
        .filter(|items| !items.is_empty());

    let has_stories = my_stories.is_some();
    let sender_for_viewer = if app.stories.stories_by_sender.contains_key(&me_str) {
        me_str.clone()
    } else {
        "me".to_string()
    };

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        if has_stories {
            app.actions.push(Action::OpenStoryViewer {
                sender: sender_for_viewer,
                index: 0,
            });
        } else {
            app.actions.push(Action::OpenPostStory);
        }
    }

    if has_stories {
        response.on_hover_text("My status\nClick to view your status update\nUse '+' in header to add another");
    } else {
        response.on_hover_text("My status\nClick to add status update");
    }

    let avatar_center = pos2(rect.center().x, rect.top() + 24.0);

    // If user has active stories, draw ring around avatar
    if has_stories {
        ui.painter().circle_stroke(
            avatar_center,
            24.0,
            Stroke::new(2.2, Color32::from_rgb(0, 168, 132)),
        );
    }

    // Draw user avatar or placeholder
    let me = app.me.clone().unwrap_or_default();
    let me_name = app.me_name.clone().unwrap_or_else(|| "You".to_owned());
    let picture = app.avatar(&me);

    ui.scope_builder(
        egui::UiBuilder::new().max_rect(Rect::from_center_size(avatar_center, vec2(42.0, 42.0))),
        |ui| {
            widgets::avatar(ui, palette, &me_name, &me, 42.0, picture.as_deref());
        },
    );

    // Draw '+' badge at bottom-right of avatar
    let badge_center = pos2(avatar_center.x + 15.0, avatar_center.y + 15.0);
    ui.painter().circle_filled(badge_center, 9.0, palette.panel);
    ui.painter().circle_filled(badge_center, 7.5, Color32::from_rgb(0, 168, 132));
    ui.painter().text(
        badge_center + vec2(0.0, -1.0),
        Align2::CENTER_CENTER,
        "+",
        egui::FontId::proportional(12.0),
        Color32::WHITE,
    );

    // Label
    let label_rect = Rect::from_min_size(
        pos2(rect.left(), rect.top() + 50.0),
        vec2(rect.width(), 16.0),
    );
    ui.painter().text(
        label_rect.center(),
        Align2::CENTER_CENTER,
        "My status",
        egui::FontId::proportional(11.0),
        palette.text,
    );
}

fn contact_story_item(
    app: &mut App,
    ui: &mut egui::Ui,
    palette: &Palette,
    contact: &ContactStories,
) {
    let item_size = vec2(54.0, 68.0);
    let (rect, response) = ui.allocate_exact_size(item_size, Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        app.actions.push(Action::OpenStoryViewer {
            sender: contact.sender.clone(),
            index: 0,
        });
    }

    let tooltip = format!(
        "{}\n{} status update{}",
        contact.sender_name,
        contact.items.len(),
        if contact.items.len() == 1 { "" } else { "s" }
    );
    response.on_hover_text(tooltip);

    let avatar_center = pos2(rect.center().x, rect.top() + 24.0);
    let ring_color = if contact.has_unviewed {
        Color32::from_rgb(0, 168, 132) // Active WhatsApp green
    } else {
        if palette.dark {
            Color32::from_gray(100)
        } else {
            Color32::from_gray(180)
        }
    };

    // Draw story ring around avatar
    ui.painter().circle_stroke(
        avatar_center,
        24.0,
        Stroke::new(2.2, ring_color),
    );

    // Draw contact avatar
    let picture = app.avatar(&contact.sender);
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(Rect::from_center_size(avatar_center, vec2(42.0, 42.0))),
        |ui| {
            widgets::avatar(ui, palette, &contact.sender_name, &contact.sender, 42.0, picture.as_deref());
        },
    );

    // Short contact first name
    let short_name = contact
        .sender_name
        .split_whitespace()
        .next()
        .unwrap_or(&contact.sender_name);
    let truncated: String = short_name.chars().take(7).collect();
    let display_text = if short_name.chars().count() > 7 {
        format!("{truncated}…")
    } else {
        truncated
    };

    let label_rect = Rect::from_min_size(
        pos2(rect.left(), rect.top() + 50.0),
        vec2(rect.width(), 16.0),
    );
    ui.painter().text(
        label_rect.center(),
        Align2::CENTER_CENTER,
        display_text,
        egui::FontId::proportional(11.0),
        palette.text,
    );
}

/// Full-screen overlay modal for viewing a story with timer progress.
pub fn viewer_show(app: &mut App, ctx: &egui::Context) {
    let Some(state) = app.story_viewer.clone() else {
        return;
    };

    let Some(items) = app.stories.stories_by_sender.get(&state.sender).cloned() else {
        app.actions.push(Action::CloseStoryViewer);
        return;
    };

    if items.is_empty() || state.index >= items.len() {
        app.actions.push(Action::CloseStoryViewer);
        return;
    }

    let current_item = items[state.index].clone();

    // Mark current item as viewed if not already
    if !current_item.viewed {
        app.stories.mark_viewed(&state.sender, &current_item.id);
        app.backend.send(crate::backend::Command::ViewStory {
            sender: state.sender.clone(),
            id: current_item.id.clone(),
        });
        let stories_file = app.dirs.state.join("stories.json");
        app.stories.save(&stories_file);
    }

    // Keyboard navigation
    let (esc, left, right, space) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::Escape),
            i.key_pressed(egui::Key::ArrowLeft),
            i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Space),
        )
    });

    if esc {
        app.actions.push(Action::CloseStoryViewer);
        return;
    }
    if left {
        app.actions.push(Action::PrevStory);
        return;
    }
    if right {
        app.actions.push(Action::NextStory);
        return;
    }
    if space {
        if let Some(viewer) = app.story_viewer.as_mut() {
            viewer.paused = !viewer.paused;
        }
    }

    // Timer progress (5.0 seconds per status)
    let elapsed = if state.paused {
        Duration::from_secs(0)
    } else {
        state.started_at.elapsed()
    };

    let story_duration = 5.0_f32;
    let progress = (elapsed.as_secs_f32() / story_duration).clamp(0.0, 1.0);

    if !state.paused && progress >= 1.0 {
        app.actions.push(Action::NextStory);
    } else if !state.paused {
        ctx.request_repaint_after(Duration::from_millis(30));
    }

    let screen = ctx.content_rect();
    let palette = app.palette;

    // Full screen overlay area
    egui::Area::new(egui::Id::new("story-viewer-surface"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());

            // Dark backdrop
            ui.painter()
                .rect_filled(screen, CornerRadius::ZERO, Color32::from_black_alpha(240));

            // Centered Story Card
            let card_width = 440.0_f32.min(screen.width() - 32.0);
            let card_height = 700.0_f32.min(screen.height() - 48.0);
            let card_rect = Rect::from_center_size(screen.center(), vec2(card_width, card_height));

            // Background of the story
            let bg_color = match &current_item.media_type {
                Some(_) => Color32::from_rgb(18, 20, 24),
                None => current_item
                    .background_argb
                    .map(argb_to_color32)
                    .unwrap_or_else(|| Color32::from_rgb(0, 168, 132)),
            };

            ui.painter()
                .rect_filled(card_rect, CornerRadius::same(16), bg_color);
            ui.painter().rect_stroke(
                card_rect,
                CornerRadius::same(16),
                Stroke::new(1.0, Color32::from_white_alpha(30)),
                egui::StrokeKind::Inside,
            );

            // Click zones for Prev / Next story
            let left_zone = Rect::from_min_max(card_rect.min, pos2(card_rect.left() + card_width * 0.35, card_rect.bottom()));
            let right_zone = Rect::from_min_max(pos2(card_rect.left() + card_width * 0.35, card_rect.top()), card_rect.max);

            let left_resp = ui.allocate_rect(left_zone, Sense::click());
            let right_resp = ui.allocate_rect(right_zone, Sense::click());

            if left_resp.clicked() {
                app.actions.push(Action::PrevStory);
            }
            if right_resp.clicked() {
                app.actions.push(Action::NextStory);
            }

            // Top Progress Bars
            let seg_spacing = 4.0;
            let total_spacing = seg_spacing * (items.len().saturating_sub(1) as f32);
            let bar_width = ((card_width - 32.0) - total_spacing) / (items.len() as f32);
            let bar_y = card_rect.top() + 14.0;

            for (i, _) in items.iter().enumerate() {
                let seg_x = card_rect.left() + 16.0 + i as f32 * (bar_width + seg_spacing);
                let seg_rect = Rect::from_min_size(pos2(seg_x, bar_y), vec2(bar_width, 3.0));

                // Background track
                ui.painter().rect_filled(
                    seg_rect,
                    CornerRadius::same(2),
                    Color32::from_white_alpha(75),
                );

                // Filled portion
                let fill_fraction = if i < state.index {
                    1.0
                } else if i == state.index {
                    progress
                } else {
                    0.0
                };

                if fill_fraction > 0.0 {
                    let filled_rect = Rect::from_min_size(
                        seg_rect.min,
                        vec2(bar_width * fill_fraction, 3.0),
                    );
                    ui.painter().rect_filled(
                        filled_rect,
                        CornerRadius::same(2),
                        Color32::WHITE,
                    );
                }
            }

            // Header: Avatar, Name, Timestamp, Close Button
            let header_rect = Rect::from_min_size(
                pos2(card_rect.left() + 16.0, card_rect.top() + 24.0),
                vec2(card_width - 32.0, 40.0),
            );

            let avatar_center = pos2(header_rect.left() + 18.0, header_rect.center().y);
            let sender_name = current_item
                .sender_name
                .as_deref()
                .unwrap_or(&current_item.sender);
            let picture = app.avatar(&current_item.sender);

            ui.scope_builder(
                egui::UiBuilder::new().max_rect(Rect::from_center_size(avatar_center, vec2(36.0, 36.0))),
                |ui| {
                    widgets::avatar(ui, &palette, sender_name, &current_item.sender, 36.0, picture.as_deref());
                },
            );

            // Name & time text
            let name_pos = pos2(header_rect.left() + 46.0, header_rect.top() + 4.0);
            ui.painter().text(
                name_pos,
                Align2::LEFT_TOP,
                sender_name,
                egui::FontId::proportional(14.0),
                Color32::WHITE,
            );

            let time_pos = pos2(header_rect.left() + 46.0, header_rect.top() + 22.0);
            let time_str = format_relative_time(current_item.timestamp);
            ui.painter().text(
                time_pos,
                Align2::LEFT_TOP,
                time_str,
                egui::FontId::proportional(11.5),
                Color32::from_white_alpha(180),
            );

            // Close button (✕)
            let close_rect = Rect::from_center_size(
                pos2(header_rect.right() - 14.0, header_rect.center().y),
                vec2(28.0, 28.0),
            );
            let close_resp = ui.allocate_rect(close_rect, Sense::click());
            if close_resp.hovered() {
                ui.painter().circle_filled(close_rect.center(), 14.0, Color32::from_white_alpha(40));
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.painter().text(
                close_rect.center(),
                Align2::CENTER_CENTER,
                "✕",
                egui::FontId::proportional(16.0),
                Color32::WHITE,
            );
            if close_resp.clicked() {
                app.actions.push(Action::CloseStoryViewer);
            }

            // Center Content
            let content_rect = Rect::from_min_max(
                pos2(card_rect.left() + 24.0, card_rect.top() + 80.0),
                pos2(card_rect.right() - 24.0, card_rect.bottom() - 40.0),
            );

            if let Some(text) = &current_item.text {
                // Large centered text status
                let font_size = if text.len() < 60 {
                    26.0
                } else if text.len() < 140 {
                    22.0
                } else {
                    18.0
                };
                let font_id = egui::FontId::proportional(font_size);

                // Word wrap text within content rect
                let galley = ui.painter().layout(
                    text.clone(),
                    font_id,
                    Color32::WHITE,
                    content_rect.width(),
                );

                let text_center = content_rect.center();
                let text_pos = pos2(
                    text_center.x - galley.size().x / 2.0,
                    text_center.y - galley.size().y / 2.0,
                );
                ui.painter().galley(text_pos, galley, Color32::WHITE);
            } else if let Some(thumb_bytes) = &current_item.thumbnail {
                // Decode thumbnail bytes
                if let Ok(image) = image::load_from_memory(thumb_bytes) {
                    let rgba = image.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let pixels = rgba.into_raw();
                    let color_img = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
                    let texture = ctx.load_texture(
                        format!("story-thumb-{}", current_item.id),
                        color_img,
                        egui::TextureOptions::LINEAR,
                    );

                    let img_size = vec2(size[0] as f32, size[1] as f32);
                    let fitted_rect = fit_inside(content_rect, img_size);

                    ui.painter().image(
                        texture.id(),
                        fitted_rect,
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }

                // Caption if present
                if let Some(caption) = &current_item.caption {
                    let cap_rect = Rect::from_min_max(
                        pos2(card_rect.left() + 16.0, card_rect.bottom() - 60.0),
                        pos2(card_rect.right() - 16.0, card_rect.bottom() - 16.0),
                    );
                    ui.painter().rect_filled(
                        cap_rect,
                        CornerRadius::same(10),
                        Color32::from_black_alpha(170),
                    );
                    ui.painter().text(
                        cap_rect.center(),
                        Align2::CENTER_CENTER,
                        caption,
                        egui::FontId::proportional(14.0),
                        Color32::WHITE,
                    );
                }
            } else {
                ui.painter().text(
                    content_rect.center(),
                    Align2::CENTER_CENTER,
                    "Status Update",
                    egui::FontId::proportional(20.0),
                    Color32::from_white_alpha(180),
                );
            }
        });
}

/// Modal dialog for composing and posting a new text status to WhatsApp.
pub fn post_modal_show(app: &mut App, ctx: &egui::Context) {
    if !app.post_story_open {
        return;
    }

    let palette = app.palette;
    let frame = Frame::new()
        .fill(palette.overlay)
        .stroke(Stroke::new(1.0, palette.outline))
        .corner_radius(CornerRadius::same(theme::RADIUS + 4))
        .inner_margin(Margin::same(20))
        .shadow(palette.modal_shadow());

    let response = egui::Modal::new(egui::Id::new("post-story-modal"))
        .frame(frame)
        .backdrop_color(palette.shadow)
        .show(ctx, |ui| {
            ui.set_width(420.0);

            // Header row
            ui.horizontal(|ui| {
                theme::text(
                    ui,
                    crate::i18n::gettext(app.locale, "Create Status"),
                    theme::bold(17.0),
                    palette.text,
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if theme::icon_button(ui, Icon::X, 18.0, palette.secondary, palette.text, "Close").clicked() {
                        app.actions.push(Action::ClosePostStory);
                    }
                });
            });

            ui.add_space(14.0);

            // Live preview card
            let selected_bg = STORY_COLORS[app.post_story_color_idx % STORY_COLORS.len()].0;
            let preview_bg = argb_to_color32(selected_bg);
            let preview_rect = ui.allocate_space(vec2(ui.available_width(), 160.0)).1;

            ui.painter().rect_filled(preview_rect, CornerRadius::same(12), preview_bg);
            ui.painter().rect_stroke(
                preview_rect,
                CornerRadius::same(12),
                Stroke::new(1.0, Color32::from_white_alpha(30)),
                egui::StrokeKind::Inside,
            );

            let preview_text = if app.post_story_text.trim().is_empty() {
                "Type a status…".to_string()
            } else {
                app.post_story_text.clone()
            };

            let preview_galley = ui.painter().layout(
                preview_text,
                egui::FontId::proportional(20.0),
                if app.post_story_text.trim().is_empty() {
                    Color32::from_white_alpha(140)
                } else {
                    Color32::WHITE
                },
                preview_rect.width() - 32.0,
            );

            let text_pos = pos2(
                preview_rect.center().x - preview_galley.size().x / 2.0,
                preview_rect.center().y - preview_galley.size().y / 2.0,
            );
            ui.painter().galley(text_pos, preview_galley, Color32::WHITE);

            ui.add_space(12.0);

            // Text input
            theme::text(ui, "Status text", theme::medium(12.0), palette.secondary);
            ui.add_space(4.0);

            let mut text = app.post_story_text.clone();
            let text_edit = egui::TextEdit::multiline(&mut text)
                .hint_text("What's on your mind?")
                .desired_rows(3)
                .desired_width(ui.available_width())
                .char_limit(700);

            let _edit_resp = ui.add(text_edit);
            if text != app.post_story_text {
                app.post_story_text = text;
            }

            ui.add_space(10.0);

            // Color picker row
            theme::text(ui, "Background color", theme::medium(12.0), palette.secondary);
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                for (idx, (color_val, name)) in STORY_COLORS.iter().enumerate() {
                    let color = argb_to_color32(*color_val);
                    let (dot_rect, dot_resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());

                    if dot_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if dot_resp.clicked() {
                        app.post_story_color_idx = idx;
                    }
                    dot_resp.on_hover_text(*name);

                    ui.painter().circle_filled(dot_rect.center(), 13.0, color);

                    if app.post_story_color_idx == idx {
                        ui.painter().circle_stroke(
                            dot_rect.center(),
                            14.5,
                            Stroke::new(2.0, palette.text),
                        );
                        ui.painter().text(
                            dot_rect.center(),
                            Align2::CENTER_CENTER,
                            "✓",
                            egui::FontId::proportional(12.0),
                            Color32::WHITE,
                        );
                    }
                }
            });

            ui.add_space(10.0);

            // Font picker row
            theme::text(ui, "Font style", theme::medium(12.0), palette.secondary);
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                for (idx, (name, _)) in STORY_FONTS.iter().enumerate() {
                    let selected = app.post_story_font_idx == idx;
                    if ui.selectable_label(selected, *name).clicked() {
                        app.post_story_font_idx = idx;
                    }
                }
            });

            ui.add_space(16.0);

            // Bottom Buttons
            ui.horizontal(|ui| {
                let char_count = format!("{}/700", app.post_story_text.chars().count());
                theme::text(ui, char_count, theme::medium(11.5), palette.secondary);

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let can_send = !app.post_story_text.trim().is_empty();

                    let send_btn = ui.add_enabled(
                        can_send,
                        egui::Button::new(
                            egui::RichText::new("Send Status")
                                .size(13.0)
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(0, 168, 132))
                        .corner_radius(CornerRadius::same(16))
                        .min_size(vec2(100.0, 32.0)),
                    );

                    if send_btn.clicked() {
                        let text = app.post_story_text.trim().to_string();
                        let bg = STORY_COLORS[app.post_story_color_idx % STORY_COLORS.len()].0;
                        let font = STORY_FONTS[app.post_story_font_idx % STORY_FONTS.len()].1;

                        app.actions.push(Action::PostTextStory {
                            text,
                            background_argb: bg,
                            font,
                        });
                        app.actions.push(Action::ClosePostStory);
                    }

                    if ui.button("Cancel").clicked() {
                        app.actions.push(Action::ClosePostStory);
                    }
                });
            });
        });

    if response.should_close() {
        app.actions.push(Action::ClosePostStory);
    }
}
