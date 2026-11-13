use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, Vec2};

#[derive(Clone, Copy)]
pub enum Icon { Sidebar, Split, Plus, Expand, Close, Folder }

pub fn draw(ui: &egui::Ui, rect: Rect, icon: Icon, color: Color32) {
    let p = ui.painter();
    let c = rect.center();
    let r = Rect::from_center_size(c, Vec2::splat(14.0));
    let stroke = Stroke::new(1.3_f32, color);
    let line = |a: Pos2, b: Pos2| { p.line_segment([a, b], stroke); };
    match icon {
        Icon::Sidebar | Icon::Split => {
            p.rect_stroke(r, 2.0, stroke, egui::StrokeKind::Inside);
            let x = if matches!(icon, Icon::Sidebar) { r.left() + 5.0 } else { c.x };
            line(egui::pos2(x, r.top()), egui::pos2(x, r.bottom()));
        }
        Icon::Plus => { line(c - egui::vec2(6.0,0.0), c + egui::vec2(6.0,0.0)); line(c - egui::vec2(0.0,6.0), c + egui::vec2(0.0,6.0)); }
        Icon::Close => { line(c-egui::vec2(4.0,4.0),c+egui::vec2(4.0,4.0)); line(c+egui::vec2(-4.0,4.0),c+egui::vec2(4.0,-4.0)); }
        Icon::Expand => {
            for (x,y) in [(-1.0,-1.0),(1.0,1.0)] {
                let corner=c+egui::vec2(x*6.0,y*6.0);
                line(corner,corner-egui::vec2(x*5.0,0.0)); line(corner,corner-egui::vec2(0.0,y*5.0));
            }
        }
        Icon::Folder => {
            p.rect_stroke(Rect::from_min_max(r.min+egui::vec2(0.0,3.0),r.max-egui::vec2(0.0,1.0)),2.0,stroke,egui::StrokeKind::Inside);
            line(r.min+egui::vec2(1.0,3.0),r.min+egui::vec2(1.0,0.0));
            line(r.min+egui::vec2(1.0,0.0),r.min+egui::vec2(6.0,0.0));
            line(r.min+egui::vec2(6.0,0.0),r.min+egui::vec2(8.0,3.0));
        }
    }
}

pub fn button(ui: &mut egui::Ui, icon: Icon, label: &str, size: Vec2) -> Response {
    let (rect, response)=ui.allocate_exact_size(size,Sense::click());
    let visuals=ui.style().interact(&response);
    if response.hovered() || response.has_focus() { ui.painter().rect_filled(rect,5.0,visuals.weak_bg_fill); }
    draw(ui,rect,icon,visuals.fg_stroke.color);
    response.widget_info(||egui::WidgetInfo::labeled(egui::WidgetType::Button,ui.is_enabled(),label));
    response.on_hover_text(label)
}

#[cfg(test)]
mod badge_tests {
    #[test]
    fn badge_stays_compact_in_a_tall_centered_header() {
        let ctx=eframe::egui::Context::default();
        let mut badge=None;
        let _=ctx.run(Default::default(),|ctx| {
            eframe::egui::CentralPanel::default().show(ctx,|ui| {
                ui.allocate_ui(eframe::egui::vec2(400.0,26.0),|ui| {
                    ui.horizontal_centered(|ui| {
                        badge=Some(super::pane_badge(ui,"LOCAL",eframe::egui::Color32::BLACK,eframe::egui::Color32::GRAY));
                    });
                });
            });
        });
        let rect=badge.unwrap();
        assert_eq!(rect.height(),17.0);
        assert!(rect.width() < 50.0);
    }
}

/// Paint at a fixed height: a Frame inside horizontal_centered stretches to the row.
pub fn pane_badge(ui: &mut egui::Ui, text: &str, foreground: Color32, background: Color32) -> Rect {
    let galley=ui.painter().layout_no_wrap(text.to_owned(),egui::FontId::proportional(10.0),foreground);
    let size=egui::vec2(galley.size().x+8.0,17.0);
    let (allocated,_)=ui.allocate_exact_size(size,Sense::hover());
    let rect=Rect::from_center_size(allocated.center(),size);
    ui.painter().rect_filled(rect,4.0,background);
    ui.painter().galley(rect.center()-galley.size()*0.5,galley,foreground);
    rect
}
