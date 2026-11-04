//! Touch widgets of the phone layout ([`crate::mobile`]): the transport row, a simple timeline
//! with the playhead fixed in the middle (drag to scrub, pinch to zoom, tap a clip to select it,
//! drag its edges to trim) and the tool bar. Automation ids: `mobile.*`.

use egui::{Align2, Color32, CornerRadius, Painter, Pos2, Rect, Sense, Shape, Stroke, pos2, vec2};
use filmcraft_project::{ClipId, ItemId};
use filmcraft_time::Tick;
use serde_json::json;

use crate::FilmcraftApp;
use crate::mobile::{Place, add_media, run, trim};
use crate::theme::Tokens;

const RULER_H: f32 = 26.0;
const HANDLE_W: f32 = 14.0;
const BG: Color32 = Color32::from_rgb(0x0c, 0x0c, 0x0e);
const VIDEO: Color32 = Color32::from_rgb(0x2a, 0x4d, 0x69);
const AUDIO: Color32 = Color32::from_rgb(0x2f, 0x6b, 0x3f);

/// An edge being dragged to trim a clip; applied when the finger lifts.
#[derive(Clone, Copy, Debug)]
pub struct TrimDrag {
    pub clip: ClipId,
    pub in_edge: bool,
    pub dx: f32,
}

#[derive(Clone, Copy)]
pub enum Ico {
    Plus,
    Play,
    Pause,
    Prev,
    Next,
    Undo,
    Redo,
    Music,
    Text,
    Cut,
    Trash,
    TrimIn,
    TrimOut,
    Library,
    Video,
    Back,
}

/// A line-art icon centred on `c`, about `2 * s` wide.
pub fn paint_icon(p: &Painter, ico: Ico, c: Pos2, s: f32, col: Color32) {
    let st = Stroke::new((s * 0.16).max(1.4), col);
    let at = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([at(a.0, a.1), at(b.0, b.1)], st);
    };
    let tri = |a: (f32, f32), b: (f32, f32), d: (f32, f32)| {
        p.add(Shape::convex_polygon(vec![at(a.0, a.1), at(b.0, b.1), at(d.0, d.1)], col, Stroke::NONE));
    };
    match ico {
        Ico::Plus => {
            line((-0.8, 0.0), (0.8, 0.0));
            line((0.0, -0.8), (0.0, 0.8));
        }
        Ico::Play => tri((-0.55, -0.85), (0.85, 0.0), (-0.55, 0.85)),
        Ico::Pause => {
            p.rect_filled(Rect::from_min_max(at(-0.6, -0.8), at(-0.2, 0.8)), 1.0, col);
            p.rect_filled(Rect::from_min_max(at(0.2, -0.8), at(0.6, 0.8)), 1.0, col);
        }
        Ico::Prev => {
            tri((0.7, -0.75), (-0.3, 0.0), (0.7, 0.75));
            line((-0.7, -0.75), (-0.7, 0.75));
        }
        Ico::Next => {
            tri((-0.7, -0.75), (0.3, 0.0), (-0.7, 0.75));
            line((0.7, -0.75), (0.7, 0.75));
        }
        Ico::Undo | Ico::Redo => {
            let m = if matches!(ico, Ico::Undo) { 1.0 } else { -1.0 };
            let pts: Vec<Pos2> = (0..=10)
                .map(|i| {
                    let a = (20.0 + 170.0 * i as f32 / 10.0).to_radians();
                    at(m * 0.8 * a.cos(), 0.1 - 0.8 * a.sin())
                })
                .collect();
            p.add(Shape::line(pts, st));
            line((m * -0.8, 0.2), (m * -0.8, -0.45));
            line((m * -0.8, 0.2), (m * -0.25, 0.2));
        }
        Ico::Music => {
            p.circle_stroke(at(-0.4, 0.55), 0.35 * s, st);
            line((-0.05, 0.5), (-0.05, -0.8));
            line((-0.05, -0.8), (0.7, -0.45));
        }
        Ico::Text => {
            line((-0.7, -0.7), (0.7, -0.7));
            line((0.0, -0.7), (0.0, 0.8));
            line((-0.25, 0.8), (0.25, 0.8));
        }
        Ico::Cut => {
            line((-0.6, -0.85), (0.6, 0.35));
            line((0.6, -0.85), (-0.6, 0.35));
            p.circle_stroke(at(-0.55, 0.65), 0.28 * s, st);
            p.circle_stroke(at(0.55, 0.65), 0.28 * s, st);
        }
        Ico::Trash => {
            line((-0.7, -0.5), (0.7, -0.5));
            line((-0.25, -0.5), (-0.25, -0.8));
            line((0.25, -0.5), (0.25, -0.8));
            line((-0.25, -0.8), (0.25, -0.8));
            line((-0.5, -0.5), (-0.4, 0.85));
            line((0.5, -0.5), (0.4, 0.85));
            line((-0.4, 0.85), (0.4, 0.85));
        }
        Ico::TrimIn => {
            line((-0.6, -0.8), (-0.6, 0.8));
            line((-0.6, -0.8), (-0.2, -0.8));
            line((-0.6, 0.8), (-0.2, 0.8));
            line((-0.3, 0.0), (0.8, 0.0));
            tri((0.0, -0.3), (-0.4, 0.0), (0.0, 0.3));
        }
        Ico::TrimOut => {
            line((0.6, -0.8), (0.6, 0.8));
            line((0.6, -0.8), (0.2, -0.8));
            line((0.6, 0.8), (0.2, 0.8));
            line((0.3, 0.0), (-0.8, 0.0));
            tri((0.0, -0.3), (0.4, 0.0), (0.0, 0.3));
        }
        Ico::Library => {
            for (x, y) in [(-0.8, -0.8), (0.05, -0.8), (-0.8, 0.05), (0.05, 0.05)] {
                p.rect_stroke(Rect::from_min_max(at(x, y), at(x + 0.75, y + 0.75)), 1.5, st, egui::StrokeKind::Inside);
            }
        }
        Ico::Video => {
            p.rect_stroke(Rect::from_min_max(at(-0.85, -0.55), at(0.35, 0.55)), 3.0, st, egui::StrokeKind::Inside);
            tri((0.45, 0.0), (0.9, -0.4), (0.9, 0.4));
        }
        Ico::Back => {
            line((0.3, -0.8), (-0.4, 0.0));
            line((-0.4, 0.0), (0.3, 0.8));
        }
    }
}

/// A flat icon button (no background, like the Premiere mobile bars); true when tapped.
pub fn icon_button(app: &mut FilmcraftApp, ui: &mut egui::Ui, rect: Rect, id: &str, tip: &str, ico: Ico, enabled: bool, accent: bool) -> bool {
    let t = app.tokens;
    let resp = ui.interact(rect, egui::Id::new(("mobile", id)), if enabled { Sense::click() } else { Sense::hover() });
    app.auto.add(&format!("mobile.{id}"), rect, tip);
    if resp.is_pointer_button_down_on() && enabled {
        ui.painter().circle_filled(rect.center(), rect.width().min(rect.height()) * 0.5, t.hover);
    }
    let col = if !enabled {
        t.text_dim.gamma_multiply(0.45)
    } else if accent {
        t.accent
    } else {
        Color32::WHITE
    };
    paint_icon(ui.painter(), ico, rect.center(), 10.0, col);
    enabled && resp.clicked()
}

/// A tool bar entry: icon over a small label.
fn tool(app: &mut FilmcraftApp, ui: &mut egui::Ui, rect: Rect, id: &str, label: &str, ico: Ico, enabled: bool) -> bool {
    let t = app.tokens;
    let resp = ui.interact(rect, egui::Id::new(("mobile", id)), if enabled { Sense::click() } else { Sense::hover() });
    app.auto.add(&format!("mobile.{id}"), rect, label);
    if resp.is_pointer_button_down_on() && enabled {
        ui.painter().rect_filled(rect.shrink(2.0), CornerRadius::same(10), t.hover);
    }
    let col = if enabled { Color32::WHITE } else { t.text_dim.gamma_multiply(0.45) };
    paint_icon(ui.painter(), ico, pos2(rect.center().x, rect.min.y + rect.height() * 0.38), 11.0, col);
    ui.painter().text(pos2(rect.center().x, rect.max.y - rect.height() * 0.2), Align2::CENTER_CENTER, label, Tokens::ui(12.0), col);
    enabled && resp.clicked()
}

/// `0:03.2`
fn clock(secs: f64) -> String {
    let s = if secs.is_finite() && secs > 0.0 { secs } else { 0.0 };
    let m = (s / 60.0).floor();
    format!("{}:{:04.1}", m as u64, s - m * 60.0)
}

pub fn transport(app: &mut FilmcraftApp, ui: &mut egui::Ui, rect: Rect) {
    let t = app.tokens;
    let ctx = ui.ctx().clone();
    ui.painter().rect_filled(rect, 0.0, BG);
    let cy = rect.center().y;
    let b = 48.0;
    let cell = |cx: f32| Rect::from_center_size(pos2(cx, cy), vec2(b, b));
    let mid = rect.center().x;
    if icon_button(app, ui, cell(mid - 56.0), "prev", "Previous cut", Ico::Prev, true, false) {
        run(app, &ctx, "playhead.prevEdit", json!({}));
    }
    let (ico, tip) = if app.playback.playing { (Ico::Pause, "Pause") } else { (Ico::Play, "Play") };
    if icon_button(app, ui, cell(mid), "play", tip, ico, true, false) {
        app.toggle_play(1.0);
    }
    if icon_button(app, ui, cell(mid + 56.0), "next", "Next cut", Ico::Next, true, false) {
        run(app, &ctx, "playhead.nextEdit", json!({}));
    }
    if icon_button(app, ui, cell(rect.max.x - 8.0 - b * 1.5), "undo", "Undo", Ico::Undo, app.session.history.can_undo(), false) {
        run(app, &ctx, "edit.undo", json!({}));
    }
    if icon_button(app, ui, cell(rect.max.x - 8.0 - b * 0.5), "redo", "Redo", Ico::Redo, app.session.history.can_redo(), false) {
        run(app, &ctx, "edit.redo", json!({}));
    }
    let head = app.session.playhead().seconds();
    let total = app.session.active_sequence().map(|q| q.duration().seconds()).unwrap_or(0.0);
    ui.painter().text(pos2(rect.min.x + 12.0, cy), Align2::LEFT_CENTER, format!("{} / {}", clock(head), clock(total)), Tokens::ui(12.0), t.text_dim);
}

struct ClipV {
    id: ClipId,
    item: ItemId,
    name: String,
    start: Tick,
    end: Tick,
    source_in: Tick,
}

struct RowV {
    video: bool,
    main: bool,
    clips: Vec<ClipV>,
}

fn rows_of(app: &FilmcraftApp) -> Vec<RowV> {
    let Some(q) = app.session.active_sequence() else { return Vec::new() };
    let view = |tr: &filmcraft_project::Track, video: bool, main: bool| RowV {
        video,
        main,
        clips: tr
            .items
            .iter()
            .map(|it| ClipV { id: it.id, item: it.item, name: it.name.clone(), start: it.start, end: it.end(), source_in: it.source_in })
            .collect(),
    };
    let mut rows = Vec::new();
    for (i, tr) in q.video_tracks.iter().enumerate().rev() {
        if i == 0 || !tr.items.is_empty() {
            rows.push(view(tr, true, i == 0));
        }
    }
    for (i, tr) in q.audio_tracks.iter().enumerate() {
        if i == 0 || !tr.items.is_empty() {
            rows.push(view(tr, false, false));
        }
    }
    rows
}

/// Ruler label step (seconds) so labels are at least ~64 px apart.
fn ruler_step(pps: f64) -> f64 {
    [0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0].into_iter().find(|s| s * pps >= 64.0).unwrap_or(1800.0)
}

pub fn timeline(app: &mut FilmcraftApp, ui: &mut egui::Ui, rect: Rect) {
    let t = app.tokens;
    let ctx = ui.ctx().clone();
    ui.painter().rect_filled(rect, 0.0, BG);
    let Some(seq_id) = app.session.state.active_sequence else { return };
    let Some(q) = app.session.active_sequence() else { return };
    let dur = q.duration().seconds().max(0.0);
    let aspect = if q.settings.height > 0 { q.settings.width as f32 / q.settings.height as f32 } else { 1.78 };
    let rows = rows_of(app);
    let head_t = app.session.playhead();
    let head = head_t.seconds();

    // zoom: fit the sequence the first time (and when the first clip arrives), pinch afterwards
    let m = &mut app.mobile;
    if !(m.pps.is_finite() && m.pps > 0.0) || m.fit_for != Some(seq_id.0) || (m.fit_dur < 0.5 && dur >= 0.5) {
        m.pps = ((rect.width() as f64 * 0.8) / dur.max(1.0)).clamp(4.0, 400.0);
        (m.fit_for, m.fit_dur) = (Some(seq_id.0), dur);
    }
    let (zoom, pinching) = ctx.input(|i| (i.zoom_delta(), i.multi_touch().is_some()));
    if zoom.is_finite() && (zoom - 1.0).abs() > 1e-3 && (pinching || ctx.input(|i| i.pointer.hover_pos().is_some_and(|p| rect.contains(p)))) {
        m.pps = (m.pps * zoom as f64).clamp(2.0, 1500.0);
    }
    let pps = m.pps;
    let cx = rect.center().x;
    let x_of = |secs: f64| -> f32 { cx + ((secs - head) * pps).clamp(-1.0e6, 1.0e6) as f32 };

    // layout of the rows
    let lanes = Rect::from_min_max(pos2(rect.min.x, rect.min.y + RULER_H), rect.max);
    let avail = (lanes.height() - 8.0).max(0.0);
    let n = rows.len().max(1) as f32;
    let main_h = if rows.len() <= 1 { avail.min(72.0) } else { (avail - (n - 1.0) * 30.0).clamp(40.0, 64.0) };
    let other_h = if rows.len() <= 1 { 0.0 } else { ((avail - main_h) / (n - 1.0)).clamp(22.0, 38.0) };
    let mut y = lanes.min.y + 4.0;
    let mut row_rects = Vec::new();
    for r in &rows {
        let h = if r.main { main_h } else { other_h.max(22.0) };
        row_rects.push(Rect::from_min_size(pos2(rect.min.x, y), vec2(rect.width(), h - 3.0)));
        y += h;
    }

    // scrub with a drag anywhere, tap to select
    let body = ui.interact(rect, egui::Id::new("mobile-timeline"), Sense::click_and_drag());
    app.auto.add("mobile.timeline", rect, "Timeline");
    if body.drag_started() && app.playback.playing {
        app.stop();
    }
    if body.dragged() && app.mobile.trim_drag.is_none() {
        let dx = body.drag_delta().x as f64;
        if dx.is_finite() {
            app.session.set_playhead(Tick::from_seconds_f64((head - dx / pps).clamp(0.0, dur)));
        }
    }
    if body.clicked()
        && let Some(pos) = body.interact_pointer_pos()
    {
        let secs = head + (pos.x - cx) as f64 / pps;
        // Thin or tiny clips are hard to hit: take the nearest clip within ~18 px.
        let tol = 18.0 / pps;
        let hit = rows
            .iter()
            .zip(&row_rects)
            .filter(|(_, rr)| pos.y >= rr.min.y - 4.0 && pos.y <= rr.max.y + 4.0)
            .flat_map(|(r, _)| r.clips.iter())
            .map(|c| {
                let (a, b) = (c.start.seconds(), c.end.seconds());
                let d = if secs < a {
                    a - secs
                } else if secs > b {
                    secs - b
                } else {
                    0.0
                };
                (d, c.id)
            })
            .filter(|(d, _)| *d <= tol)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id);
        match hit {
            Some(id) if app.session.state.selection.first() != Some(&id) => app.session.state.selection = vec![id],
            _ => app.session.state.selection.clear(),
        }
    }
    let selected = app.session.state.selection.first().copied();

    // ruler
    let rp = ui.painter().with_clip_rect(Rect::from_min_max(rect.min, pos2(rect.max.x, rect.min.y + RULER_H)));
    let step = ruler_step(pps);
    let first = ((head - (cx - rect.min.x) as f64 / pps) / step).floor().max(0.0);
    for k in 0..200 {
        let secs = (first + k as f64) * step;
        let x = x_of(secs);
        if x > rect.max.x + 40.0 {
            break;
        }
        if x >= rect.min.x - 40.0 {
            rp.line_segment([pos2(x, rect.min.y + RULER_H - 6.0), pos2(x, rect.min.y + RULER_H - 1.0)], Stroke::new(1.0, t.text_dim));
            rp.text(pos2(x + 3.0, rect.min.y + RULER_H * 0.45), Align2::LEFT_CENTER, clock(secs), Tokens::ui(10.5), t.text_dim);
        }
    }

    // clips
    let ppp = ctx.pixels_per_point();
    let trim_drag = app.mobile.trim_drag;
    let mut handle_rects: Option<(ClipId, Rect, Rect, Rect)> = None;
    for (r, rr) in rows.iter().zip(&row_rects) {
        let lane = ui.painter().with_clip_rect(*rr);
        for c in &r.clips {
            let (mut x0, mut x1) = (x_of(c.start.seconds()), x_of(c.end.seconds()));
            if x1 < rect.min.x - 4.0 || x0 > rect.max.x + 4.0 {
                continue;
            }
            // never thinner than a few pixels, so every clip can be seen and tapped
            x1 = x1.max(x0 + 6.0);
            // the clip being trimmed follows the finger
            if let Some(d) = trim_drag.filter(|d| d.clip == c.id) {
                if d.in_edge {
                    x0 = (x0 + d.dx).min(x1 - 8.0);
                } else {
                    x1 = (x1 + d.dx).max(x0 + 8.0);
                }
            }
            let cr = Rect::from_min_max(pos2(x0, rr.min.y), pos2(x1, rr.max.y));
            app.auto.add(&format!("mobile.clip.{}", c.id.0), cr.intersect(rect), &c.name);
            lane.rect_filled(cr, CornerRadius::same(4), if r.video { VIDEO } else { AUDIO });
            if r.video && rr.height() >= 36.0 {
                let tile_h = rr.height() - 4.0;
                let tile_w = (tile_h * aspect).clamp(18.0, 160.0);
                let skip = ((rect.min.x - x0) / tile_w).floor().max(0.0) as u32;
                for k in skip..skip + 40 {
                    let tx = x0 + k as f32 * tile_w;
                    if tx >= x1 || tx > rect.max.x {
                        break;
                    }
                    let w = (x1 - tx).min(tile_w);
                    let at = c.source_in + Tick::from_seconds_f64(((tx + w * 0.5 - x0) as f64 / pps).max(0.0));
                    let px = (tile_w * ppp).round().clamp(32.0, 512.0) as u32;
                    if let Some((tex, _)) = app.thumbnail(&ctx, c.item, at, px) {
                        let tr = Rect::from_min_size(pos2(tx, rr.min.y + 2.0), vec2(w, tile_h));
                        lane.image(tex, tr, Rect::from_min_max(pos2(0.0, 0.0), pos2(w / tile_w, 1.0)), Color32::WHITE);
                    }
                }
            } else if cr.width() > 20.0 {
                lane.text(pos2(cr.min.x + 6.0, cr.center().y), Align2::LEFT_CENTER, &c.name, Tokens::ui(11.0), Color32::WHITE);
            }
            if selected == Some(c.id) {
                lane.rect_stroke(cr, CornerRadius::same(4), Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Inside);
                let h = |x: f32| Rect::from_center_size(pos2(x, cr.center().y), vec2(HANDLE_W * 2.0, cr.height().max(28.0)));
                handle_rects = Some((c.id, h(cr.min.x), h(cr.max.x), cr));
            }
        }
    }
    if rows.iter().all(|r| r.clips.is_empty()) {
        ui.painter().text(rect.center() + vec2(0.0, 8.0), Align2::CENTER_CENTER, "Tap Video to add your first clip", Tokens::ui(14.0), t.text_dim);
    }

    // trim handles of the selected clip (above the scrub area)
    if let Some((id, rin, rout, cr)) = handle_rects {
        for (in_edge, hr) in [(true, rin), (false, rout)] {
            let resp = ui.interact(hr, egui::Id::new(("mobile-handle", in_edge)), Sense::drag());
            app.auto.add(if in_edge { "mobile.handle.in" } else { "mobile.handle.out" }, hr, "Trim handle");
            if resp.dragged() {
                let dx = resp.drag_delta().x;
                let prev = app.mobile.trim_drag.filter(|d| d.clip == id && d.in_edge == in_edge).map_or(0.0, |d| d.dx);
                if dx.is_finite() {
                    app.mobile.trim_drag = Some(TrimDrag { clip: id, in_edge, dx: prev + dx });
                }
            }
            if resp.drag_stopped()
                && let Some(d) = app.mobile.trim_drag.take().filter(|d| d.clip == id && d.in_edge == in_edge)
            {
                let ticks = Tick::from_seconds_f64(d.dx as f64 / pps).0;
                if ticks != 0 {
                    trim(app, &ctx, id, if in_edge { "in" } else { "out" }, ticks);
                }
            }
            let x = if in_edge { cr.min.x } else { cr.max.x };
            let bar = Rect::from_center_size(pos2(x + if in_edge { 4.0 } else { -4.0 }, cr.center().y), vec2(8.0, cr.height().clamp(24.0, 40.0)));
            ui.painter().with_clip_rect(rect).rect_filled(bar, CornerRadius::same(3), Color32::WHITE);
        }
    }

    // playhead (fixed in the middle) and the current time
    let pl = ui.painter().with_clip_rect(rect);
    pl.line_segment([pos2(cx, rect.min.y + RULER_H - 2.0), pos2(cx, rect.max.y)], Stroke::new(2.0, Color32::WHITE));
    let tag = Rect::from_center_size(pos2(cx, rect.min.y + RULER_H * 0.5), vec2(58.0, 20.0));
    pl.rect_filled(tag, CornerRadius::same(10), Color32::WHITE);
    pl.text(tag.center(), Align2::CENTER_CENTER, clock(head), Tokens::semibold(11.5), Color32::BLACK);
    if app.playback.playing {
        ctx.request_repaint();
    }
}

pub fn toolbar(app: &mut FilmcraftApp, ui: &mut egui::Ui, rect: Rect) {
    let ctx = ui.ctx().clone();
    ui.painter().rect_filled(rect, 0.0, BG);
    let head = app.session.playhead();
    let selected = app.session.state.selection.first().copied();
    let clip = selected.and_then(|id| app.session.active_sequence().and_then(|q| q.find_item(id)).map(|(_, it)| (it.id, it.start, it.end())));
    let editing = clip.is_some();
    let inside = clip.is_some_and(|(_, s, e)| s <= head && head < e);
    let items: Vec<(&str, &str, Ico, bool)> = if editing {
        vec![
            ("done", "Done", Ico::Back, true),
            ("split", "Split", Ico::Cut, inside),
            ("trim-in", "Trim in", Ico::TrimIn, inside),
            ("trim-out", "Trim out", Ico::TrimOut, inside),
            ("delete", "Delete", Ico::Trash, true),
        ]
    } else {
        vec![
            ("add", "Video", Ico::Video, true),
            ("music", "Music", Ico::Music, true),
            ("text", "Text", Ico::Text, true),
            ("edit", "Edit", Ico::Cut, true),
            ("library", "Library", Ico::Library, true),
        ]
    };
    let w = rect.width() / items.len().max(1) as f32;
    let mut tapped = None;
    for (i, (id, label, ico, enabled)) in items.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.min.x + i as f32 * w, rect.min.y + 4.0), vec2(w, rect.height() - 8.0));
        if tool(app, ui, r, id, label, *ico, *enabled) {
            tapped = Some(*id);
        }
    }
    let Some(tap) = tapped else { return };
    match tap {
        "add" => add_media(app, &ctx, Place::Append),
        "music" => add_media(app, &ctx, Place::Playhead),
        "text" => run(app, &ctx, "graphics.newText", json!({})),
        "library" => app.mobile.library_open = true,
        "edit" => {
            let under = app.session.active_sequence().and_then(|q| {
                q.video_tracks
                    .iter()
                    .rev()
                    .chain(q.audio_tracks.iter())
                    .flat_map(|tr| tr.items.iter())
                    .find(|it| it.start <= head && head < it.end())
                    .map(|it| it.id)
            });
            match under {
                Some(id) => app.session.state.selection = vec![id],
                None => app.ui.status = "Tap a clip to select it".into(),
            }
        }
        "done" => app.session.state.selection.clear(),
        "split" => {
            run(app, &ctx, "sequence.addEdit", json!({"time": head.0}));
            app.session.state.selection.clear();
        }
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
                app.session.state.selection.clear();
            }
        }
        _ => {}
    }
}
