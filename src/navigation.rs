/// Wrap navigation at the edges, including an empty workspace.
pub fn adjacent(current: usize, count: usize, forward: bool) -> usize {
    if count == 0 { return 0; }
    let current = current.min(count - 1);
    if forward { (current + 1) % count } else { (current + count - 1) % count }
}

pub fn visible_panes(count: usize, focused: usize, zoomed: bool) -> Vec<usize> {
    if count == 0 { return vec![]; }
    if zoomed { vec![focused.min(count - 1)] } else { (0..count).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wraps_projects_and_panes_in_both_directions() {
        assert_eq!(adjacent(0, 4, false), 3);
        assert_eq!(adjacent(3, 4, true), 0);
        assert_eq!(adjacent(1, 4, true), 2);
        assert_eq!(adjacent(0, 0, true), 0);
        assert_eq!(adjacent(0, 1, false), 0);
    }
    #[test]
    fn zoom_follows_focus_and_restores_all_panes() {
        assert_eq!(visible_panes(5, 3, true), vec![3]);
        assert_eq!(visible_panes(5, 4, true), vec![4]);
        assert_eq!(visible_panes(5, 4, false), vec![0, 1, 2, 3, 4]);
        assert!(visible_panes(0, 0, true).is_empty());
    }
}

#[cfg(test)]
mod layout_tests {
    #[test]
    fn dense_layout_tiles_without_gaps_and_finds_focus_before_render() {
        let area = eframe::egui::Rect::from_min_size(eframe::egui::pos2(151.0, 40.0), eframe::egui::vec2(900.0, 600.0));
        let rects = super::pane_rects(area, 5, 0, false);
        assert_eq!(rects.len(), 5);
        assert_eq!(rects.iter().map(|(_, r)| r.area()).sum::<f32>(), area.area());
        assert_eq!(super::pane_at(&rects, rects[4].1.center()), Some(4));
        assert_eq!(super::pane_rects(area, 5, 4, true), vec![(4, area)]);
    }
}

/// Allocate continuous rows, with a partial last row using the full width.
pub fn pane_rects(area: eframe::egui::Rect, count: usize, focused: usize, zoomed: bool) -> Vec<(usize, eframe::egui::Rect)> {
    use eframe::egui::{Rect, pos2};
    let visible = visible_panes(count, focused, zoomed);
    if visible.is_empty() { return vec![]; }
    let columns = (visible.len() as f32).sqrt().ceil() as usize;
    let rows = visible.len().div_ceil(columns);
    let mut result = Vec::with_capacity(visible.len());
    for (row, indices) in visible.chunks(columns).enumerate() {
        for (column, index) in indices.iter().enumerate() {
            result.push((*index, Rect::from_min_max(
                pos2(area.left() + area.width() * column as f32 / indices.len() as f32, area.top() + area.height() * row as f32 / rows as f32),
                pos2(area.left() + area.width() * (column + 1) as f32 / indices.len() as f32, area.top() + area.height() * (row + 1) as f32 / rows as f32))));
        }
    }
    result
}
pub fn pane_at(rects: &[(usize, eframe::egui::Rect)], point: eframe::egui::Pos2) -> Option<usize> {
    rects.iter().find(|(_, rect)| rect.contains(point)).map(|(index, _)| *index)
}
