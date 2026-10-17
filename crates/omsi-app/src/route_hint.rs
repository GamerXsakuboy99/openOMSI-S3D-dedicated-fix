//! The hint over the minimap: which line and route to set on the IBIS for the course under way
//! (`Line 5 · Route 02`, and where it leads), as the depot file has them ([`schedule::ibis_target`],
//! the same entry the typing job follows).
//!
//! [ROLLBACK routehint-1] It is there while the course needs a change - the IBIS (or a roller blind) shows
//! another route - goes by itself once line and route are right, and comes back with the next course
//! that asks for another. The cross in its corner puts it away for the course it was dismissed
//! on only. Nothing is shown while a menu is up, with the interface hidden, with the city map
//! open or while the typing job is working the IBIS, nor without a minimap to put it on.
//!
//! It is drawn with `omsi-ui` over the game's picture as the quick menu is (`quick_menu.rs`).

use crate::app::App;
use crate::schedule;
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::{Atlas, Color, Draw, Fonts, Gpu, Layer, Painter, Rect, Weight};
use std::sync::Arc;

const PANEL: Color = Color::rgba(14, 16, 20, 0.88);
const EDGE: Color = Color::rgba(244, 127, 48, 0.9);
const TEXT: Color = Color::rgba(240, 240, 240, 1.0);
const DIM: Color = Color::rgba(240, 240, 240, 0.62);
const CROSS_HOVER: Color = Color::rgba(78, 84, 92, 0.96);

/// What a course asks of the IBIS, worked out once for it.
#[derive(Clone)]
struct Asked {
    /// The line as the timetable calls it ("5", "5E").
    line: String,
    /// The route within the line (the route code's last two digits), where the depot file has one.
    route: Option<u32>,
    /// The depot file's name of the route, and where the course ends.
    route_name: String,
    terminus: String,
    target: omsi_sim::ibis::Target,
    /// A hand-cranked roller blind is turned to the course, not typed.
    blind: bool,
}

pub(crate) struct RouteHint {
    /// The hint is showing (the mouse needs the cross).
    pub(crate) shown: bool,
    /// The course the hint is for (see `course_key`), and the one the cross put away.
    key: Option<String>,
    dismissed: Option<String>,
    asked: Option<(String, Option<Asked>)>,
    cross: Rect,
    consumed: bool,
    gpu: Option<(Gpu, wgpu::TextureFormat)>,
    fonts: Option<Fonts>,
    atlas: Atlas,
    painter: Painter,
}

impl RouteHint {
    pub(crate) fn new() -> RouteHint {
        RouteHint {
            shown: false,
            key: None,
            dismissed: None,
            asked: None,
            cross: Rect::new(0.0, 0.0, 0.0, 0.0),
            consumed: false,
            gpu: None,
            fonts: None,
            atlas: Atlas::new(1024),
            painter: Painter::new(),
        }
    }

    /// The window's graphics went away: the picture is made again with the next frame.
    pub(crate) fn drop_gpu(&mut self) {
        self.gpu = None;
        self.atlas = Atlas::new(1024);
    }

    /// Draw the hint painted over the frame (`view`, `w` x `h`).
    pub(crate) fn render(&mut self, r: &omsi_render::Renderer, view: &wgpu::TextureView, w: u32, h: u32) {
        if !self.shown || self.painter.is_empty() {
            return;
        }
        let format = r.format();
        if self.gpu.as_ref().map(|g| g.1 != format).unwrap_or(true) {
            self.gpu = Some((Gpu::new(&r.device, format, 1, self.atlas.size), format));
            self.atlas.mark_all_dirty();
        }
        let Some((gpu, _)) = self.gpu.as_mut() else { return };
        gpu.upload(&r.device, &r.queue, 0, &self.painter.verts);
        gpu.upload_atlas(&r.queue, &mut self.atlas);
        let layers = [Layer::flat([0.0, 0.0, w as f32, h as f32], 0.0, 1.0)];
        let draws = [Draw { buffer: 0, range: 0..self.painter.len(), layer: 0, texture: 0 }];
        let mut enc = r.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("route hint") });
        gpu.render(&r.device, &r.queue, &mut enc, view, (w, h), None, &layers, &draws);
        r.queue.submit([enc.finish()]);
    }
}

/// Whether the IBIS (or the roller blind) of `v` already shows what `a` asks for: the route of the
/// depot file when the course has one (it says the line as well), else the destination.
fn is_set(v: &omsi_sim::VehicleInstance, hof: &omsi_vehicle::Hof, a: &Asked) -> bool {
    if a.blind {
        return schedule::roller_blind_row(v, hof) == usize::try_from(a.target.terminus_index).ok();
    }
    match a.target.route_index {
        Some(ri) => v.var("IBIS_RouteIndex").is_some_and(|x| x.round() as i32 == ri),
        None => v.var("IBIS_TerminusIndex").is_some_and(|x| x.round() as i32 == a.target.terminus_index),
    }
}

impl App {
    /// The course under way and what it asks of the IBIS, while a hint is due: (the course's key,
    /// what it asks). None when there is nothing to show now.
    fn route_hint_due(&mut self) -> Option<(String, Asked)> {
        if self.game_menu.is_some() || self.chooser.is_some() || self.list_kind.is_some() || self.editor.is_some() || self.gui_hidden {
            return None;
        }
        self.navigator.as_ref()?.screen_rect()?;
        let p = self.player.as_ref()?;
        // (the typing job is at it: the IBIS is being set)
        if p.ibis_typist.is_some() {
            return None;
        }
        let duty = self.duty.as_ref()?;
        let trip = duty.trips.get(duty.trip_index)?;
        let hof: Arc<omsi_vehicle::Hof> = p.vehicle.host.hof.clone()?;
        let key = format!("{}|{}|{}|{}|{}", duty.line, duty.tour, duty.first_trip + duty.trip_index, trip.terminus, hof.name);
        if self.route_hint.asked.as_ref().map(|a| a.0.as_str()) != Some(key.as_str()) {
            let stops: Vec<&str> = trip.stops.iter().map(|s| s.name.as_str()).collect();
            let asked = schedule::ibis_target(&hof, &trip.line, &trip.terminus, &stops, None).map(|target| Asked {
                line: trip.line.trim().to_string(),
                route: target.route,
                route_name: target
                    .route_index
                    .and_then(|i| usize::try_from(i).ok())
                    .and_then(|i| hof.info_trips.get(i))
                    .map(|t| t.name.trim().to_string())
                    .unwrap_or_default(),
                terminus: trip.terminus.trim().to_string(),
                target,
                blind: schedule::has_roller_blind(&p.vehicle),
            });
            self.route_hint.asked = Some((key.clone(), asked));
        }
        let asked = self.route_hint.asked.as_ref().and_then(|a| a.1.clone())?;
        if self.route_hint.dismissed.as_deref() == Some(key.as_str()) || is_set(&p.vehicle, &hof, &asked) {
            return None;
        }
        Some((key, asked))
    }

    /// Lay the hint out for a picture of `w` x `h` physical pixels and paint it (drawn by
    /// `RouteHint::render` once the picture is in the frame).
    pub(crate) fn route_hint_prepare(&mut self, w: u32, h: u32) {
        self.route_hint.shown = false;
        self.route_hint.key = None;
        self.route_hint.painter.clear();
        let due = self.route_hint_due();
        let Some((key, asked)) = due else {
            self.route_hint.consumed = false;
            return;
        };
        let Some(nav) = self.navigator.as_ref().and_then(|n| n.screen_rect()) else { return };
        let (wf, hf) = (w as f32, h as f32);
        let dpi = self.window.as_ref().map(|win| win.scale_factor() as f32).unwrap_or(1.0).max(0.5);
        let u = dpi * ((hf / dpi) / 1000.0).clamp(0.8, 1.5);
        let cursor = Vec2::new(self.cursor.0, self.cursor.1);

        let title = omsi_ui::tr(if asked.blind { "Turn the roller blind to" } else { "Set on the IBIS" }).to_string();
        let mut head = format!("{} {}", omsi_ui::tr("Line"), asked.line);
        if let Some(r) = asked.route.filter(|_| !asked.blind) {
            head.push_str(&format!("   \u{b7}   {} {:02}", omsi_ui::tr("Route"), r));
        }
        let mut sub = format!("\u{2192} {}", asked.terminus);
        if !asked.route_name.is_empty() && !asked.route_name.eq_ignore_ascii_case(&asked.terminus) && !asked.blind {
            sub.push_str(&format!("   ({})", asked.route_name));
        }

        let q = &mut self.route_hint;
        let fonts = q.fonts.get_or_insert_with(Fonts::new);
        let (ts, hs, ss) = (12.0 * u, 18.0 * u, 12.0 * u);
        let pad = 12.0 * u;
        let cross_size = 24.0 * u;
        let tw = fonts.width(&head, hs, Weight::Bold)
            .max(fonts.width(&sub, ss, Weight::Regular))
            .max(fonts.width(&title, ts, Weight::Regular) + cross_size + 8.0 * u);
        let (bw, bh) = ((tw + 2.0 * pad).min(wf - 20.0 * u).max(120.0 * u), 2.0 * pad + 16.0 * u + 26.0 * u + 18.0 * u);
        // over the minimap, centred on it; under it where it has no room above
        let (x0, y0, x1, y1) = (nav[0], nav[1], nav[2], nav[3]);
        let gap = 8.0 * u;
        let bx = (((x0 + x1) * 0.5) - bw * 0.5).clamp(6.0 * u, (wf - bw - 6.0 * u).max(6.0 * u));
        let by = if y0 - gap - bh >= 4.0 * u { y0 - gap - bh } else { (y1 + gap).min(hf - bh - 4.0 * u) };
        let panel = Rect::new(bx, by, bw, bh);
        let cross = Rect::new(panel.right() - cross_size - 6.0 * u, panel.y + 6.0 * u, cross_size, cross_size);
        q.cross = cross;
        let on_cross = cross.contains(cursor);

        let pt = &mut q.painter;
        q.atlas.begin_frame();
        let atlas = &mut q.atlas;
        pt.rounded(panel, 12.0 * u, PANEL);
        pt.rounded_border(panel, 12.0 * u, 2.0 * u, EDGE);
        pt.text_in(atlas, fonts, &title, ts, Weight::Regular, Rect::new(panel.x + pad, panel.y + pad, panel.w - 2.0 * pad - cross_size, 16.0 * u), Align::Left, DIM);
        pt.text_in(atlas, fonts, &head, hs, Weight::Bold, Rect::new(panel.x + pad, panel.y + pad + 16.0 * u, panel.w - 2.0 * pad, 26.0 * u), Align::Left, TEXT);
        pt.text_in(atlas, fonts, &sub, ss, Weight::Regular, Rect::new(panel.x + pad, panel.y + pad + 42.0 * u, panel.w - 2.0 * pad, 18.0 * u), Align::Left, DIM);
        if on_cross {
            pt.circle(cross.center(), cross_size * 0.5, CROSS_HOVER);
        }
        pt.icon(atlas, "close", cross.center(), 16.0 * u, TEXT);
        q.shown = true;
        q.key = Some(key);
    }

    /// The left button over the picture: a click on the hint's cross puts the hint away for this
    /// course (true: taken). A click anywhere else goes on to the game.
    pub(crate) fn route_hint_click(&mut self, pressed: bool) -> bool {
        let h = &mut self.route_hint;
        if !h.shown {
            h.consumed = false;
            return false;
        }
        let p = Vec2::new(self.cursor.0, self.cursor.1);
        if pressed {
            if h.cross.contains(p) {
                h.consumed = true;
                return true;
            }
            return false;
        }
        if !h.consumed {
            return false;
        }
        h.consumed = false;
        if h.cross.contains(p) {
            h.dismissed = h.key.clone();
            h.shown = false;
        }
        true
    }
}
