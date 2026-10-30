//! Phone layout (narrow windows): program monitor on top, a transport row, the timeline, and a
//! row of large touch buttons at the bottom (add media, split, trim, delete, music, text, export).
//! Wider windows keep the docked desktop layout. Automation ids: `mobile.*`.

use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, pos2, vec2};
use filmcraft_project::{ClipId, ItemId};
use filmcraft_time::{Tick, TimeDisplay, format_time};
use serde_json::{Value, json};

use crate::FilmcraftApp;
use crate::dock::PanelKind;
use crate::state::Mode;
use crate::theme::Tokens;

/// Windows narrower than this (in points) use the phone layout.
pub const MAX_WIDTH: f32 = 720.0;
const TOP_H: f32 = 48.0;
const PLAY_H: f32 = 56.0;
const TOOLS_H: f32 = 72.0;
const TOUCH: f32 = 44.0;

pub fn is_mobile(full: Rect) -> bool {
    full.width() < MAX_WIDTH
}

/// Where media picked with the add buttons goes once it is imported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Video and stills: after the last clip of the sequence.
    Append,
    /// Music and sound: at the playhead.
    Playhead,
}

#[derive(Default)]
pub struct MobileState {
    pub place: Option<Place>,
    pub library_open: bool,
    /// Set while the phone layout draws the Program monitor: picture only.
    pub compact_monitor: bool,
}

impl FilmcraftApp {
    /// Put freshly imported project items on the timeline when an add button asked for it
    /// (the browser's file pickers are asynchronous: this runs when the import is done).
    pub fn place_imported(&mut self, items: &[u64]) {
        let Some(mode) = self.mobile.place.take() else { return };
        // A fresh project has no sequence: make one from the first clip (its size and frame rate)
        // and put everything on it.
        if self.session.active_sequence().is_none() {
            if let Err(e) = self.session.execute("file.newSequenceFromClip", json!({"items": items})) {
                self.ui.status = e.to_string();
            }
            return;
        }
        for id in items {
            let at = match mode {
                Place::Append => self.session.active_sequence().map(|q| q.duration()).unwrap_or_default(),
                Place::Playhead => self.session.playhead(),
            };
            let mut params = json!({"item": id, "time": at.0, "insert": false});
            if mode == Place::Playhead {
                // music goes on its own audio track, never over the sound of the video
                let dur = self.session.project.item(ItemId(*id)).map(|i| i.duration()).unwrap_or_default();
                if let Some(track) = self.audio_track_for(at, dur) {
                    params["track"] = json!(track);
                }
            }
            if let Err(e) = self.session.execute("timeline.place", params) {
                self.ui.status = e.to_string();
            }
        }
    }

    /// An unlocked audio track that is free over `at .. at + dur`; adds one when there is none.
    fn audio_track_for(&mut self, at: Tick, dur: Tick) -> Option<u64> {
        let free = |s: &filmcraft_engine::Session| {
            s.active_sequence()
                .and_then(|q| q.audio_tracks.iter().find(|t| !t.locked && t.items.iter().all(|i| i.end() <= at || i.start >= at + dur)).map(|t| t.id.0))
        };
        let track = match free(&self.session) {
            Some(id) => id,
            None => {
                self.session.execute("sequence.addTracks", json!({"video": 0, "audio": 1})).ok()?;
                free(&self.session)?
            }
        };
        // Music stays put when the picture is rippled (trim, delete) and the other way round.
        let _ = self.session.execute("timeline.setTrack", json!({"track": track, "syncLock": false}));
        Some(track)
    }
}

pub fn show(app: &mut FilmcraftApp, ui: &mut egui::Ui, full: Rect) {
    let t = app.tokens;
    let ctx = ui.ctx().clone();
    ui.painter().rect_filled(full, 0.0, t.app_bg);
    let top = Rect::from_min_size(full.min, vec2(full.width(), TOP_H));
    top_bar(app, ui, top);
    if app.ui.mode != Mode::Edit {
        let body = Rect::from_min_max(pos2(full.min.x, top.max.y), full.max);
        match app.ui.mode {
            Mode::Import => crate::panels::import_mode::show(app, ui, body),
            _ => export_page(app, ui, body),
        }
        return;
    }
    let tools = Rect::from_min_max(pos2(full.min.x, full.max.y - TOOLS_H), full.max);
    let body_h = (tools.min.y - top.max.y).max(0.0);
    let monitor_h = (body_h * 0.36).clamp(120.0, 360.0).min(body_h);
    let monitor = Rect::from_min_size(pos2(full.min.x, top.max.y), vec2(full.width(), monitor_h));
    let play = Rect::from_min_size(pos2(full.min.x, monitor.max.y), vec2(full.width(), PLAY_H.min(body_h - monitor_h).max(0.0)));
    let timeline = Rect::from_min_max(pos2(full.min.x, play.max.y), pos2(full.max.x, tools.min.y));

    // Touch-sized tracks and a narrow header column.
    let tl = &mut app.ui.timeline;
    tl.header_w = 100.0;
    tl.video_track_h = tl.video_track_h.max(64.0);
    tl.audio_track_h = tl.audio_track_h.max(56.0);
    app.ui.show_menu_bar = false;

    app.mobile.compact_monitor = true;
    panel(app, ui, monitor, PanelKind::Program);
    app.mobile.compact_monitor = false;
    playbar(app, ui, play);
    panel(app, ui, timeline, PanelKind::Timeline);
    pinch_zoom(app, &ctx, timeline);
    toolbar(app, ui, tools);
    if app.mobile.library_open {
        library(app, ui, full);
    }
    crate::panels::dialogs::show(app, &ctx);
}

fn panel(app: &mut FilmcraftApp, ui: &mut egui::Ui, rect: Rect, p: PanelKind) {
    if rect.width() < 1.0 || rect.height() < 1.0 {
        return;
    }
    app.auto.add(&format!("panel.{}", p.id()), rect, p.title());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(("mobile-panel", p.id())));
    child.set_clip_rect(rect);
    crate::panels::show(app, &mut child, p, rect);
}

/// Two-finger pinch zooms the timeline around its current view.
fn pinch_zoom(app: &mut FilmcraftApp, ctx: &egui::Context, area: Rect) {
    let (delta, over) = ctx.input(|i| (i.zoom_delta(), i.pointer.hover_pos().is_none_or(|p| area.contains(p))));
    if over && delta.is_finite() && (delta - 1.0).abs() > 1e-3 {
        let v = &mut app.ui.timeline;
        v.target_pps = (v.target_pps * delta as f64).clamp(0.5, 4000.0);
    }
}

/// A rounded touch button; returns whether it was tapped.
fn button(app: &mut FilmcraftApp, ui: &mut egui::Ui, rect: Rect, id: &str, label: &str, enabled: bool, primary: bool) -> bool {
    let t = app.tokens;
    let resp = ui.interact(rect, egui::Id::new(("mobile", id)), if enabled { Sense::click() } else { Sense::hover() });
    app.auto.add(&format!("mobile.{id}"), rect, label);
    let fill = if primary && enabled {
        t.accent
    } else if resp.is_pointer_button_down_on() && enabled {
        t.hover
    } else {
        Color32::from_rgb(0x34, 0x35, 0x3a)
    };
    let ink = if !enabled {
        t.text_dim.gamma_multiply(0.5)
    } else if primary {
        Color32::WHITE
    } else {
        t.text
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(8), fill);
    p.text(rect.center(), Align2::CENTER_CENTER, label, Tokens::ui(13.0), ink);
    enabled && resp.clicked()
}

fn run(app: &mut FilmcraftApp, ctx: &egui::Context, id: &str, params: Value) {
    if let Err(e) = crate::menus::invoke(app, ctx, id, params) {
        app.ui.status = e;
    }
}

fn top_bar(app: &mut FilmcraftApp, ui: &mut egui::Ui, rect: Rect) {
    let t = app.tokens;
    let ctx = ui.ctx().clone();
    ui.painter().rect_filled(rect, 0.0, t.header_bg);
    let pad = 6.0;
    let mut x = rect.min.x + pad;
    let cy = rect.center().y;
    if app.ui.mode != Mode::Edit {
        let r = Rect::from_min_size(pos2(x, cy - TOUCH / 2.0), vec2(96.0, TOUCH));
        if button(app, ui, r, "back", "< Edit", true, false) {
            app.ui.mode = Mode::Edit;
        }
        return;
    }
    let undo = Rect::from_min_size(pos2(x, cy - TOUCH / 2.0), vec2(TOUCH + 10.0, TOUCH));
    if button(app, ui, undo, "undo", "Undo", app.session.history.can_undo(), false) {
        run(app, &ctx, "edit.undo", json!({}));
    }
    x = undo.max.x + 4.0;
    let redo = Rect::from_min_size(pos2(x, cy - TOUCH / 2.0), vec2(TOUCH + 10.0, TOUCH));
    if button(app, ui, redo, "redo", "Redo", app.session.history.can_redo(), false) {
        run(app, &ctx, "edit.redo", json!({}));
    }
    let export = Rect::from_min_size(pos2(rect.max.x - pad - 88.0, cy - TOUCH / 2.0), vec2(88.0, TOUCH));
    if button(app, ui, export, "export", "Export", true, true) {
        app.ui.mode = Mode::Export;
    }
    let title = Rect::from_min_max(pos2(redo.max.x + 8.0, rect.min.y), pos2(export.min.x - 8.0, rect.max.y));
    let name = format!("{}{}", app.session.project.name, if app.session.is_dirty() { " •" } else { "" });
    let p = ui.painter().with_clip_rect(title);
    p.text(title.center(), Align2::CENTER_CENTER, name, Tokens::ui(14.0), t.text_dim);
}

fn playbar(app: &mut FilmcraftApp, ui: &mut egui::Ui, rect: Rect) {
    if rect.height() < 8.0 {
        return;
    }
    let t = app.tokens;
    ui.painter().rect_filled(rect, 0.0, t.app_bg);
    let rate = app.session.sequence_rate();
    let head = app.session.playhead();
    let total = app.session.active_sequence().map(|q| q.duration()).unwrap_or_default();
    let cy = rect.center().y;
    let w = 48.0;
    let mut x = rect.min.x + 6.0;
    let slot = |x: &mut f32| {
        let r = Rect::from_min_size(pos2(*x, cy - TOUCH / 2.0), vec2(w, TOUCH));
        *x += w + 4.0;
        r
    };
    let r = slot(&mut x);
    if button(app, ui, r, "start", "Start", true, false) {
        app.session.set_playhead(Tick::ZERO);
    }
    let r = slot(&mut x);
    let label = if app.playback.playing { "Pause" } else { "Play" };
    if button(app, ui, r, "play", label, true, true) {
        app.toggle_play(1.0);
    }
    let r = slot(&mut x);
    if button(app, ui, r, "end", "End", true, false) {
        app.session.set_playhead(total);
    }
    let tc = format_time(head, rate, false, TimeDisplay::Timecode, 48000);
    let all = format_time(total, rate, false, TimeDisplay::Timecode, 48000);
    ui.painter().text(pos2(rect.max.x - 10.0, cy), Align2::RIGHT_CENTER, format!("{tc}\n{all}"), Tokens::ui(12.0), t.text);
}

/// The clip a trim, split or delete applies to: a selected clip under the playhead, else the first
/// clip under it (top video track first).
fn clip_under_playhead(app: &FilmcraftApp) -> Option<(ClipId, Tick, Tick)> {
    let q = app.session.active_sequence()?;
    let at = app.session.playhead();
    let hit = |s: Tick, e: Tick| s <= at && at < e;
    for id in &app.session.state.selection {
        if let Some((_, it)) = q.find_item(*id)
            && hit(it.start, it.end())
        {
            return Some((it.id, it.start, it.end()));
        }
    }
    q.video_tracks
        .iter()
        .rev()
        .chain(q.audio_tracks.iter())
        .flat_map(|tr| tr.items.iter())
        .find(|it| hit(it.start, it.end()))
        .map(|it| (it.id, it.start, it.end()))
}

fn add_media(app: &mut FilmcraftApp, ctx: &egui::Context, place: Place) {
    app.mobile.place = Some(place);
    let id = if place == Place::Playhead { "file.importAudio" } else { "file.import" };
    run(app, ctx, id, json!({}));
}

/// Trim a clip edge to the playhead, closing the gap when the other tracks allow it.
fn trim(app: &mut FilmcraftApp, ctx: &egui::Context, clip: ClipId, edge: &str, delta: i64) {
    let ripple = json!({"clip": clip.0, "edge": edge, "mode": "ripple", "delta": delta});
    if app.session.execute("timeline.trim", ripple).is_err() {
        run(app, ctx, "timeline.trim", json!({"clip": clip.0, "edge": edge, "mode": "regular", "delta": delta}));
    }
}

fn toolbar(app: &mut FilmcraftApp, ui: &mut egui::Ui, rect: Rect) {
    let t = app.tokens;
    let ctx = ui.ctx().clone();
    ui.painter().rect_filled(rect, 0.0, t.header_bg);
    let clip = clip_under_playhead(app);
    let have_clip = clip.is_some();
    let head = app.session.playhead();
    let tools: [(&str, &str, bool); 9] = [
        ("add", "+ Video", true),
        ("music", "+ Music", true),
        ("split", "Split", have_clip),
        ("trim-in", "Trim in", have_clip),
        ("trim-out", "Trim out", have_clip),
        ("delete", "Delete", have_clip),
        ("library", "Library", true),
        ("text", "Text", true),
        ("fit", "Fit", true),
    ];

    let w = 74.0;
    let inner = Rect::from_min_max(pos2(rect.min.x, rect.min.y + 6.0), pos2(rect.max.x, rect.max.y - 6.0));
    let mut tapped: Option<&str> = None;
    let mut tools_ui = ui.new_child(egui::UiBuilder::new().max_rect(inner).id_salt("mobile-tools-ui"));
    egui::ScrollArea::horizontal().id_salt("mobile-tools").scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(
        &mut tools_ui,
        |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (id, label, enabled) in tools {
                    let (r, _) = ui.allocate_exact_size(vec2(w, inner.height()), Sense::hover());
                    if button(app, ui, r, id, label, enabled, false) {
                        tapped = Some(id);
                    }
                }
            });
        },
    );
    let Some(tap) = tapped else { return };
    match tap {
        "add" => add_media(app, &ctx, Place::Append),
        "music" => add_media(app, &ctx, Place::Playhead),
        "library" => app.mobile.library_open = true,
        "split" => run(app, &ctx, "sequence.addEdit", json!({"time": head.0})),
        "trim-in" => {
            if let Some((id, start, _)) = clip {
                trim(app, &ctx, id, "in", (head - start).0);
            }
        }
        "trim-out" => {
            if let Some((id, _, end)) = clip {
                trim(app, &ctx, id, "out", (head - end).0);
            }
        }
        "delete" => {
            if let Some((id, _, _)) = clip {
                app.session.state.selection = vec![id];
                // close the gap when the other tracks allow it, else leave it
                if app.session.execute("edit.rippleDelete", json!({})).is_err() {
                    run(app, &ctx, "edit.clear", json!({}));
                }
            }
        }
        "text" => run(app, &ctx, "graphics.newText", json!({})),
        "fit" => app.ui.timeline.fit_pending = true,
        _ => {}
    }
}

/// A sheet over the lower half with the Project bin and "Add to timeline" for the selected items.
fn library(app: &mut FilmcraftApp, ui: &mut egui::Ui, full: Rect) {
    let t = app.tokens;
    let ctx = ui.ctx().clone();
    let sheet = Rect::from_min_max(pos2(full.min.x, full.min.y + full.height() * 0.38), full.max);
    ui.painter().rect_filled(sheet, CornerRadius { nw: 12, ne: 12, sw: 0, se: 0 }, t.panel_bg);
    ui.painter().rect_stroke(sheet, CornerRadius { nw: 12, ne: 12, sw: 0, se: 0 }, Stroke::new(1.0, t.separator), egui::StrokeKind::Inside);
    let bar = Rect::from_min_size(sheet.min, vec2(sheet.width(), TOUCH + 12.0));
    let close = Rect::from_min_size(pos2(bar.min.x + 8.0, bar.min.y + 6.0), vec2(88.0, TOUCH));
    if button(app, ui, close, "library-close", "Close", true, false) {
        app.mobile.library_open = false;
    }
    let selected = app.session.state.project_selection.clone();
    let add = Rect::from_min_size(pos2(bar.max.x - 8.0 - 150.0, bar.min.y + 6.0), vec2(150.0, TOUCH));
    if button(app, ui, add, "library-add", "Add to timeline", !selected.is_empty(), true) {
        app.mobile.place = Some(Place::Append);
        let ids: Vec<u64> = selected.iter().map(|i| i.0).collect();
        app.place_imported(&ids);
        app.mobile.library_open = false;
    }
    let body = Rect::from_min_max(pos2(sheet.min.x, bar.max.y), sheet.max);
    ui.set_clip_rect(ui.clip_rect());
    let _ = &ctx;
    panel(app, ui, body, PanelKind::Project);
}

/// Export on a phone: the default preset (H.264 MP4, sequence size and frame rate), a file name,
/// one big button and the progress. The full Export screen stays on wider windows.
fn export_page(app: &mut FilmcraftApp, ui: &mut egui::Ui, body: Rect) {
    use std::sync::atomic::Ordering;
    let t = app.tokens;
    let Some(q) = app.session.active_sequence() else {
        crate::dock::placeholder(ui, body, &t, "Add a video first");
        return;
    };
    let (w, h, rate, dur) = (q.settings.width, q.settings.height, q.settings.frame_rate, q.duration());
    crate::panels::export_mode::sync_sequence(app);
    // The sequence is named after its first clip: "clip.mp4" would export as "clip.mp4.mp4".
    let file = std::path::Path::new(&app.ui.export.file_name);
    if let (Some(stem), Some(ext)) = (file.file_stem().and_then(|s| s.to_str()), file.extension().and_then(|e| e.to_str()))
        && filmcraft_media::VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
    {
        app.ui.export.file_name = stem.to_string();
    }
    ui.painter().rect_filled(body, 0.0, t.app_bg);
    let pad = 16.0;
    let col = Rect::from_min_max(pos2(body.min.x + pad, body.min.y + pad), pos2(body.max.x - pad, body.max.y - pad));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(col).id_salt("mobile-export"));
    child.set_clip_rect(col);
    child.label(egui::RichText::new("Export video").size(20.0).color(t.text));
    child.add_space(10.0);
    child.label(egui::RichText::new("File name").color(t.text_dim));
    let name = child.add_sized(vec2(col.width(), 40.0), egui::TextEdit::singleline(&mut app.ui.export.file_name).font(Tokens::ui(16.0)));
    app.auto.add("mobile.export.fileName", name.rect, "File name");
    child.add_space(10.0);
    let tc = format_time(dur, rate, false, TimeDisplay::Timecode, 48000);
    child.label(egui::RichText::new(format!("H.264 MP4 · {w}×{h} · {tc}")).color(t.text));
    child.label(egui::RichText::new("The file is saved to your downloads.").color(t.text_dim));
    child.add_space(14.0);

    let running = app.session.jobs.iter().rev().find(|j| !j.progress.finished.load(Ordering::Relaxed)).cloned();
    let go = Rect::from_min_size(pos2(col.min.x, child.min_rect().max.y + 6.0), vec2(col.width(), 52.0));
    if let Some(job) = running {
        let f = job.progress.fraction().clamp(0.0, 1.0);
        ui.painter().rect_filled(go, CornerRadius::same(8), t.separator);
        ui.painter().rect_filled(Rect::from_min_size(go.min, vec2(go.width() * f, go.height())), CornerRadius::same(8), t.accent);
        ui.painter().text(go.center(), Align2::CENTER_CENTER, format!("Exporting… {:.0}%", f * 100.0), Tokens::ui(16.0), Color32::WHITE);
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(150));
        let cancel = Rect::from_min_size(pos2(col.min.x, go.max.y + 10.0), vec2(col.width(), TOUCH));
        if button(app, ui, cancel, "export-cancel", "Cancel", true, false) {
            job.progress.cancel.store(true, Ordering::Relaxed);
        }
    } else if button(app, ui, go, "export-go", "Export", true, true) {
        let r = app.session.execute("file.exportMedia", crate::panels::export_mode::export_params(app));
        app.ui.status = match r {
            Ok(v) => format!("Exporting {}", v["path"].as_str().unwrap_or("")),
            Err(e) => e.to_string(),
        };
    }
    if !app.ui.status.is_empty() {
        let msg = app.ui.status.clone();
        let r = Rect::from_min_size(pos2(col.min.x, go.max.y + 70.0), vec2(col.width(), 60.0));
        ui.painter().with_clip_rect(r).text(r.left_top(), Align2::LEFT_TOP, msg, Tokens::ui(13.0), t.text_dim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_windows_use_the_phone_layout() {
        let r = |w: f32| Rect::from_min_size(pos2(0.0, 0.0), vec2(w, 800.0));
        assert!(is_mobile(r(390.0)));
        assert!(is_mobile(r(MAX_WIDTH - 1.0)));
        assert!(!is_mobile(r(MAX_WIDTH)));
        assert!(!is_mobile(r(1600.0)));
    }
}
