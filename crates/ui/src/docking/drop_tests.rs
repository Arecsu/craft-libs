//! Drop targets of the default profile over frames: a group body's thirds, the tab outline,
//! the fallback where a side has no room, and the root edge band.

use std::collections::HashMap;

use super::testing::DockDriver;
use super::*;
use crate::layout::{SplitAxis, SplitSize};
use egui::{Color32, Pos2, Rect, pos2, vec2};

/// Panel bodies by their rects; `rules` are the permissions of panels that differ from the
/// default, `min_width` every panel's least width.
#[derive(Default)]
struct Bodies {
    bodies: HashMap<u32, Rect>,
    rules: HashMap<u32, Permissions>,
    min_width: f32,
}

impl DockContent<u32> for Bodies {
    fn body(&mut self, ui: &mut egui::Ui, panel: &u32) {
        self.bodies.insert(*panel, ui.max_rect());
    }
    fn permissions(&self, panel: &u32) -> Permissions {
        self.rules.get(panel).copied().unwrap_or_default()
    }
    fn limits(&self, _: &u32) -> PanelLimits {
        PanelLimits { min: vec2(self.min_width, 0.0), ..PanelLimits::default() }
    }
}

type Driver = DockDriver<u32, Bodies>;

/// Frames `[1]` and `[2, 3]` side by side, each half of the area.
fn frames() -> Layout<u32> {
    Layout {
        root: Some(Node::Split {
            axis: SplitAxis::Horizontal,
            size: SplitSize::Ratio(0.5),
            first: Box::new(Node::Tabs { panels: vec![1], active: 0 }),
            second: Box::new(Node::Tabs { panels: vec![2, 3], active: 1 }),
        }),
        floating: Vec::new(),
    }
}

fn driver(rules: &[(u32, Permissions)]) -> Driver {
    DockDriver::new(frames(), Bodies { rules: rules.iter().copied().collect(), ..Bodies::default() })
}

/// The body of `panel` and its group's rect, the 26 pt strip included.
fn group(d: &Driver, panel: u32) -> (Rect, Rect) {
    let body = d.content.bodies.get(&panel).copied().unwrap_or_else(|| panic!("panel {panel} not drawn"));
    (body, Rect::from_min_max(pos2(body.left(), body.top() - 26.0), body.max))
}

fn tab(d: &Driver, panel: u32) -> Pos2 {
    d.hit(&Hit::Tab(panel)).unwrap_or_else(|| panic!("tab {panel}")).center()
}

/// The fill of the last frame's rect shape at `rect` that has no stroke.
fn fill_at(d: &Driver, rect: Rect) -> Option<Color32> {
    d.shapes().iter().find_map(|clipped| match &clipped.shape {
        egui::Shape::Rect(shape) if shape.rect == rect && shape.stroke.width == 0.0 && shape.fill != Color32::TRANSPARENT => Some(shape.fill),
        _ => None,
    })
}

/// The 4 pt line along `rect`'s edge on `zone`'s side, inside it.
fn line(rect: Rect, zone: Zone) -> Rect {
    match zone {
        Zone::Left => Rect::from_x_y_ranges(rect.left()..=(rect.left() + 4.0), rect.y_range()),
        Zone::Right => Rect::from_x_y_ranges((rect.right() - 4.0)..=rect.right(), rect.y_range()),
        Zone::Top => Rect::from_x_y_ranges(rect.x_range(), rect.top()..=(rect.top() + 4.0)),
        Zone::Bottom | Zone::Center => Rect::from_x_y_ranges(rect.x_range(), (rect.bottom() - 4.0)..=rect.bottom()),
    }
}

fn accent() -> Color32 {
    DockStyle::default().accent
}

#[test]
fn a_drop_on_a_groups_outer_third_docks_beside_it_with_a_line_and_its_middle_joins_with_an_outline() {
    let d = driver(&[]);
    let (body, rect) = group(&d, 1);
    let (w, h) = (body.width(), body.height());
    let line = |zone| line(rect, zone);
    for (zone, at) in [
        (Zone::Left, pos2(body.left() + w * 0.3, body.center().y)),
        (Zone::Right, pos2(body.right() - w * 0.3, body.center().y)),
        (Zone::Top, pos2(body.center().x, body.top() + h * 0.3)),
        (Zone::Bottom, pos2(body.center().x, body.bottom() - h * 0.3)),
        // In the top left corner, the top is relatively nearer.
        (Zone::Top, pos2(body.left() + w * 0.25, body.top() + h * 0.2)),
    ] {
        let mut d = driver(&[]);
        d.hold(tab(&d, 3), at);
        assert!(d.filled(line(zone)), "{zone:?}: a line along the group's whole edge");
        assert!(!d.filled(zone_rect(rect, zone).unwrap().shrink(1.0)), "{zone:?}: no filled half");
        assert!(!d.outlined(rect.shrink(1.0)), "{zone:?}");
        let out = d.release(at);
        assert_eq!(out.actions, vec![Action::Move { panel: 3, anchor: 1, placement: Placement::Split(zone) }], "{zone:?}");
    }
    // The middle and the tab strip join: the group's outline with a light fill, no line.
    for at in [body.center(), pos2(rect.center().x, rect.top() + 13.0)] {
        let mut d = driver(&[]);
        d.hold(tab(&d, 3), at);
        assert!(d.outlined(rect.shrink(1.0)), "{at:?}");
        assert_eq!(fill_at(&d, rect.shrink(1.0)), Some(accent().gamma_multiply(0.08)), "{at:?}: a light fill");
        assert!([Zone::Left, Zone::Right, Zone::Top, Zone::Bottom].into_iter().all(|z| !d.filled(line(z))), "{at:?}");
        let out = d.release(at);
        assert!(
            matches!(&out.actions[..], [Action::Move { panel: 3, anchor: 1, placement: Placement::Tab { .. } | Placement::Split(Zone::Center) }]),
            "{:?}",
            out.actions
        );
    }
    // A group's drag bar previews with the same painter.
    let mut d = driver(&[]);
    let bar = d.hit(&Hit::DragBar(2)).unwrap().center();
    let at = pos2(body.left() + w * 0.3, body.center().y);
    d.hold(bar, at);
    assert!(d.filled(line(Zone::Left)));
    assert!(!d.filled(zone_rect(rect, Zone::Left).unwrap().shrink(1.0)));
    let mut d = driver(&[]);
    d.hold(bar, body.center());
    assert!(d.outlined(rect.shrink(1.0)));
}

#[test]
fn a_group_that_takes_no_tabs_maps_its_whole_body_to_its_edges() {
    let canvas = Permissions { accepts_tabs: false, ..Permissions::default() };
    let d = driver(&[(1, canvas)]);
    let (body, rect) = group(&d, 1);
    for (zone, at) in [(Zone::Left, body.center() - vec2(10.0, 0.0)), (Zone::Top, body.center() - vec2(0.0, 10.0))] {
        let mut d = driver(&[(1, canvas)]);
        d.hold(tab(&d, 3), at);
        assert!(d.filled(line(rect, zone)), "{zone:?}");
        let out = d.release(at);
        assert_eq!(out.actions, vec![Action::Move { panel: 3, anchor: 1, placement: Placement::Split(zone) }], "{zone:?}");
    }
    // Its strip joins nothing.
    let mut d = driver(&[(1, canvas)]);
    let out = d.drag(tab(&d, 3), pos2(rect.center().x, rect.top() + 13.0));
    assert!(!out.actions.iter().any(|a| matches!(a, Action::Move { anchor: 1, placement: Placement::Tab { .. }, .. })), "{:?}", out.actions);
}

#[test]
fn a_side_without_room_joins_as_a_tab_or_offers_nothing() {
    // Two 400 pt frames of panels at least 300 wide: neither has room for two halves side by
    // side, but both have room for two halves stacked.
    let narrow = |rules: &[(u32, Permissions)]| {
        DockDriver::new(frames(), Bodies { rules: rules.iter().copied().collect(), min_width: 300.0, ..Bodies::default() })
    };
    let d = narrow(&[]);
    let (body, rect) = group(&d, 1);
    let left = pos2(body.left() + body.width() * 0.25, body.center().y);
    let mut d = narrow(&[]);
    d.hold(tab(&d, 3), left);
    assert!(d.outlined(rect.shrink(1.0)), "the tab outline");
    let out = d.release(left);
    assert_eq!(out.actions, vec![Action::Move { panel: 3, anchor: 1, placement: Placement::Split(Zone::Center) }]);
    let top = pos2(body.center().x, body.top() + body.height() * 0.2);
    let mut d = narrow(&[]);
    let out = d.drag(tab(&d, 3), top);
    assert_eq!(out.actions, vec![Action::Move { panel: 3, anchor: 1, placement: Placement::Split(Zone::Top) }]);
    // A group that takes no tabs offers nothing there.
    let mut d = narrow(&[(1, Permissions { accepts_tabs: false, ..Permissions::default() })]);
    d.hold(tab(&d, 3), left);
    assert!(!d.outlined(rect.shrink(1.0)));
    let out = d.release(left);
    assert!(!out.actions.iter().any(|a| matches!(a, Action::Move { anchor: 1, .. })), "{:?}", out.actions);
}

#[test]
fn the_root_edge_band_is_28_pt_and_previews_a_line_along_the_layouts_edge() {
    let d = driver(&[]);
    let (body, _) = group(&d, 1);
    let x = body.center().x;
    let mut d = driver(&[]);
    d.hold(tab(&d, 3), pos2(x, 573.0));
    assert!(d.filled(Rect::from_x_y_ranges(0.0..=800.0, 596.0..=600.0)), "a line along the layout's whole bottom edge");
    let out = d.release(pos2(x, 573.0));
    assert!(matches!(&out.actions[..], [Action::Move { panel: 3, placement: Placement::Edge(Zone::Bottom), .. }]), "{:?}", out.actions);
    // Past the band, the group's own bottom third.
    let mut d = driver(&[]);
    let out = d.drag(tab(&d, 3), pos2(x, 571.0));
    assert_eq!(out.actions, vec![Action::Move { panel: 3, anchor: 1, placement: Placement::Split(Zone::Bottom) }]);
    // The band reaches only 4 pt past its line into a tab strip, which joins the group.
    let mut d = driver(&[]);
    let out = d.drag(tab(&d, 3), pos2(x, 12.0));
    assert!(matches!(&out.actions[..], [Action::Move { panel: 3, anchor: 1, placement: Placement::Tab { .. } }]), "{:?}", out.actions);
    let mut d = driver(&[]);
    let out = d.drag(tab(&d, 3), pos2(x, 7.0));
    assert!(matches!(&out.actions[..], [Action::Move { panel: 3, placement: Placement::Edge(Zone::Top), .. }]), "{:?}", out.actions);
}

fn tabs(panels: &[u32]) -> Node<u32> {
    Node::Tabs { panels: panels.to_vec(), active: 0 }
}

fn split(axis: SplitAxis, size: SplitSize, first: Node<u32>, second: Node<u32>) -> Node<u32> {
    Node::Split { axis, size, first: Box::new(first), second: Box::new(second) }
}

/// Side frame `[1]`, canvas 0 and side frame `[2, 3]` side by side, 200, 400 and 200 pt wide.
fn canvas_columns() -> Layout<u32> {
    let sides = split(SplitAxis::Horizontal, SplitSize::FixedSecond(200.0), tabs(&[0]), Node::Tabs { panels: vec![2, 3], active: 1 });
    Layout { root: Some(split(SplitAxis::Horizontal, SplitSize::FixedFirst(200.0), tabs(&[1]), sides)), floating: Vec::new() }
}

fn canvas_driver(canvas: Permissions, style: impl Fn(&mut DockStyle) + 'static) -> Driver {
    DockDriver::new(canvas_columns(), Bodies { rules: [(0, canvas)].into(), ..Bodies::default() }).with_style(style)
}

fn moves(out: &DockOutput<u32>) -> bool {
    out.actions.iter().any(|a| matches!(a, Action::Move { .. } | Action::MovePanel { .. }))
}

#[test]
fn between_side_frames_the_outer_edge_band_covers_only_the_canvas_column() {
    let d = canvas_driver(Permissions::PROTECTED, |_| {});
    let (canvas, _) = group(&d, 0);
    let (side, _) = group(&d, 1);
    let at = pos2(canvas.center().x, 590.0);
    let mut d = canvas_driver(Permissions::PROTECTED, |_| {});
    d.hold(tab(&d, 3), at);
    assert!(d.filled(line(canvas, Zone::Bottom)), "a line along the canvas column's bottom");
    assert!(!d.filled(Rect::from_x_y_ranges(0.0..=800.0, 596.0..=600.0)), "no line along the window's bottom");
    let out = d.release(at);
    assert_eq!(out.actions, vec![Action::Move { panel: 3, anchor: 0, placement: Placement::Edge(Zone::Bottom) }]);
    // The side frames keep their full height.
    let column = split(SplitAxis::Vertical, SplitSize::Ratio(0.75), tabs(&[0]), tabs(&[3]));
    let sides = split(SplitAxis::Horizontal, SplitSize::FixedSecond(200.0), column, tabs(&[2]));
    assert_eq!(d.layout.root, Some(split(SplitAxis::Horizontal, SplitSize::FixedFirst(200.0), tabs(&[1]), sides)));
    // Under a side frame, its own bottom third.
    let mut d = canvas_driver(Permissions::PROTECTED, |_| {});
    let out = d.drag(tab(&d, 3), pos2(side.center().x, 590.0));
    assert_eq!(out.actions, vec![Action::Move { panel: 3, anchor: 1, placement: Placement::Split(Zone::Bottom) }]);
}

#[test]
fn between_docks_the_outer_edge_band_covers_the_part_between_them() {
    let dock = |side, panels: &[u32]| Node::Dock { side, columns: vec![Column::new(vec![Group::new(panels.to_vec(), 0)], 200.0)] };
    let middle = split(SplitAxis::Vertical, SplitSize::Ratio(0.7), tabs(&[0]), tabs(&[4]));
    let inner = split(SplitAxis::Horizontal, SplitSize::FixedSecond(200.0), middle.clone(), dock(Side::End, &[2, 3]));
    let layout =
        Layout { root: Some(split(SplitAxis::Horizontal, SplitSize::FixedFirst(200.0), dock(Side::Start, &[1]), inner)), floating: Vec::new() };
    let driver = || DockDriver::new(layout.clone(), Bodies { rules: [(0, Permissions::PROTECTED)].into(), ..Bodies::default() });
    let d = driver();
    let (canvas, _) = group(&d, 0);
    let (timeline, _) = group(&d, 4);
    let area = Rect::from_x_y_ranges(canvas.x_range(), (canvas.top() - 26.0)..=timeline.bottom());
    let at = pos2(canvas.center().x, 590.0);
    let mut d = driver();
    d.hold(tab(&d, 3), at);
    assert!(d.filled(line(area, Zone::Bottom)), "a line along the bottom between the docks");
    let out = d.release(at);
    assert!(matches!(&out.actions[..], [Action::Move { panel: 3, placement: Placement::Edge(Zone::Bottom), .. }]), "{:?}", out.actions);
    let Some(Node::Split { second, .. }) = &d.layout.root else { panic!("{:?}", d.layout) };
    let Node::Split { first, .. } = &**second else { panic!("{:?}", d.layout) };
    assert_eq!(**first, split(SplitAxis::Vertical, SplitSize::Ratio(0.75), middle, tabs(&[3])));
}

#[test]
fn drop_edges_limit_the_outer_edges_and_the_canvas_sides() {
    let left_right = |style: &mut DockStyle| style.drop_edges = Sides::LEFT_RIGHT;
    let d = canvas_driver(Permissions::PROTECTED, left_right);
    // The canvas has no tab strip: its body is its frame.
    let (canvas, _) = group(&d, 0);
    let (side, _) = group(&d, 1);
    // Without top and bottom, no outer edge takes a drop: left and right are docks here.
    for at in [pos2(canvas.center().x, 590.0), pos2(canvas.left() + 5.0, 300.0), pos2(canvas.right() - 5.0, 300.0)] {
        let mut d = canvas_driver(Permissions::PROTECTED, left_right);
        d.hold(tab(&d, 3), at);
        assert!(!d.filled(line(canvas, Zone::Bottom)), "{at:?}");
        let out = d.release(at);
        assert!(!moves(&out), "{at:?}: {:?}", out.actions);
    }
    // A side frame keeps all four sides.
    for (zone, y) in [(Zone::Bottom, 590.0), (Zone::Top, side.top() + side.height() * 0.2)] {
        let mut d = canvas_driver(Permissions::PROTECTED, left_right);
        let out = d.drag(tab(&d, 3), pos2(side.center().x, y));
        assert_eq!(out.actions, vec![Action::Move { panel: 3, anchor: 1, placement: Placement::Split(zone) }], "{zone:?}");
    }
    // No drop edges: no outer edge takes a drop.
    for at in [pos2(canvas.center().x, 590.0), pos2(canvas.center().x, 5.0), pos2(canvas.left() + 5.0, 300.0), pos2(canvas.right() - 5.0, 300.0)] {
        let mut d = canvas_driver(Permissions::PROTECTED, |style| style.drop_edges = Sides::NONE);
        let out = d.drag(tab(&d, 3), at);
        assert!(!moves(&out), "{at:?}: {:?}", out.actions);
    }
}

#[test]
fn a_canvas_that_takes_splits_takes_them_only_on_its_drop_edges() {
    let beside = Permissions { accepts_splits: true, ..Permissions::PROTECTED };
    let d = canvas_driver(beside, |_| {});
    let (body, _) = group(&d, 0);
    // Nearer the bottom than the left, and right of center nearer the right than the top.
    let low = pos2(body.left() + body.width() * 0.3, body.bottom() - 40.0);
    let high = pos2(body.right() - body.width() * 0.3, body.top() + 40.0);
    for (edges, zones) in [
        (DockStyle::default().drop_edges, [Some(Zone::Bottom), Some(Zone::Top)]),
        (Sides::LEFT_RIGHT, [Some(Zone::Left), Some(Zone::Right)]),
        (Sides::NONE, [None, None]),
    ] {
        for (at, zone) in [low, high].into_iter().zip(zones) {
            let mut d = canvas_driver(beside, move |style| style.drop_edges = edges);
            let out = d.drag(tab(&d, 3), at);
            match zone {
                Some(zone) => assert_eq!(out.actions, vec![Action::Move { panel: 3, anchor: 0, placement: Placement::Split(zone) }], "{edges:?}"),
                None => assert!(!moves(&out), "{edges:?}: {:?}", out.actions),
            }
        }
    }
}
