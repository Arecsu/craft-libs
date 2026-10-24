//! Position already shaped text within a fixed-width reading area.

use std::sync::Arc;

use egui::{Align, Galley, Rect, epaint::text::PlacedRow, pos2, vec2};

/// Align each row inside `width`, without changing its glyphs, wrapping, or logical text.
///
/// This is useful for read-only comments, captions and messages whose text must reach the
/// container's edges. `LayoutJob` alignment ordinarily uses the widest text row instead.
/// The caller supplies the row alignment, for example from paragraph direction in its shaper.
/// This helper uses only stock egui APIs and does not implement bidirectional shaping.
///
/// The returned galley uses a left-edge anchor (`job.halign = Align::Min`) and its logical
/// rectangle is exactly the requested reading area. Ink can extend outside that rectangle:
/// oversized right-aligned rows overflow to the left, and centered rows overflow on both sides.
/// `mesh_bounds` describes the complete ink, including that overflow. Apply clipping separately
/// when required. Positions snap down to physical pixels so right-aligned rows do not cross the
/// right edge through nearest-pixel rounding.
///
/// Use the returned galley for painting; per-row alignment is not represented by its single
/// `LayoutJob` alignment, so editable text should retain its original galley for hit testing.
/// Zero width is valid and may place ink entirely outside the logical rectangle.
/// Invalid widths leave the input untouched. Row positions are copy-on-write; the original
/// shared glyph meshes remain shared, and a cached galley is never modified.
#[must_use]
pub fn align_galley_rows(mut galley: Arc<Galley>, width: f32, mut align: impl FnMut(&PlacedRow) -> Align) -> Arc<Galley> {
    if !width.is_finite() || width < 0.0 {
        return galley;
    }
    let positioned = Arc::make_mut(&mut galley);
    if positioned.job.halign != Align::Min {
        Arc::make_mut(&mut positioned.job).halign = Align::Min;
    }
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(width, positioned.rect.height()));
    let mut mesh_bounds = Rect::NOTHING;
    for row in &mut positioned.rows {
        let spare = f64::from(width) - f64::from(row.size.x);
        let ppp = f64::from(positioned.pixels_per_point);
        row.pos.x = ((f64::from(align(row).to_factor()) * spare * ppp).floor() / ppp) as f32;
        mesh_bounds |= row.visuals.mesh_bounds.translate(row.pos.to_vec2());
    }
    positioned.rect = rect;
    positioned.mesh_bounds = mesh_bounds;
    galley
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> Arc<Galley> {
        layout_text("Short\nA longer line\nEnd", 1.0, Align::Min)
    }

    fn layout_text(text: &str, ppp: f32, halign: Align) -> Arc<Galley> {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.run_ui(egui::RawInput::default(), |_| {}).drop_without_applying_deltas();
        let mut job = egui::text::LayoutJob::simple(text.into(), egui::FontId::proportional(14.0), egui::Color32::BLACK, f32::INFINITY);
        job.halign = halign;
        ctx.fonts_mut(|fonts| fonts.layout_job(job))
    }

    #[test]
    fn independent_row_alignment_preserves_shaping_and_cached_geometry() {
        let original = layout();
        let positions: Vec<_> = original.rows.iter().map(|row| row.pos).collect();
        let mut i = 0;
        let aligned = align_galley_rows(original.clone(), 400.0, |_| {
            let align = match i {
                0 => Align::Max,
                1 => Align::Min,
                _ => Align::Center,
            };
            i += 1;
            align
        });
        assert_eq!(i, 3);
        assert_eq!(original.rows.iter().map(|row| row.pos).collect::<Vec<_>>(), positions);
        assert!(Arc::ptr_eq(&original.job, &aligned.job));
        assert_eq!(aligned.num_vertices, original.num_vertices);
        assert_eq!(aligned.num_indices, original.num_indices);
        for (index, (old, new)) in original.rows.iter().zip(&aligned.rows).enumerate() {
            assert!(Arc::ptr_eq(&old.row, &new.row), "glyphs and meshes must remain shared");
            assert_eq!(old.pos.y, new.pos.y);
            let factor = match index {
                0 => 1.0,
                1 => 0.0,
                _ => 0.5,
            };
            assert!((new.pos.x - (400.0 - new.size.x) * factor).abs() < 1.0);
            let bounds = new.visuals.mesh_bounds.translate(new.pos.to_vec2());
            assert!(aligned.mesh_bounds.contains_rect(bounds));
        }
        assert_eq!(aligned.size().x, 400.0);
        assert_eq!(aligned.size().y, original.size().y);
    }

    #[test]
    fn invalid_widths_preserve_the_input() {
        let original = layout();
        for width in [f32::NAN, f32::INFINITY, -1.0] {
            let aligned = align_galley_rows(original.clone(), width, |_| Align::Max);
            assert!(Arc::ptr_eq(&original, &aligned));
        }
    }

    #[test]
    fn overwide_rows_keep_the_requested_reading_edge() {
        let original = layout_text("unbreakable-long-token", 2.0, Align::Min);
        let width = 23.3;
        assert!(original.rows[0].size.x > width);
        for alignment in [Align::Min, Align::Center, Align::Max] {
            let aligned = align_galley_rows(original.clone(), width, |_| alignment);
            let row = &aligned.rows[0];
            let expected = (width - row.size.x) * alignment.to_factor();
            assert!(row.pos.x <= expected + 0.0001);
            assert!(expected - row.pos.x < 0.5);
            assert_eq!(row.pos.x * 2.0, (row.pos.x * 2.0).floor());
            if alignment == Align::Max {
                assert!(row.pos.x < 0.0);
                assert!(row.rect().right() <= width + 0.0001);
            }
            assert_eq!(aligned.rect.left(), 0.0);
            assert_eq!(aligned.rect.width(), width);
            assert!(aligned.mesh_bounds.contains_rect(row.visuals.mesh_bounds.translate(row.pos.to_vec2())));
        }
    }

    #[test]
    fn fractional_width_and_nonleft_input_normalize_the_anchor() {
        for original_align in [Align::Min, Align::Center, Align::Max] {
            let original = layout_text("Short", 2.0, original_align);
            let aligned = align_galley_rows(original.clone(), 163.3, |_| Align::Max);
            assert_eq!(original.job.halign, original_align);
            assert_eq!(aligned.job.halign, Align::Min);
            assert_eq!(aligned.rect.left(), 0.0);
            assert_eq!(aligned.rect.right(), 163.3);
            for row in &aligned.rows {
                assert_eq!(row.pos.x * 2.0, (row.pos.x * 2.0).floor());
                assert!(row.rect().right() <= 163.3);
                assert!(163.3 - row.rect().right() < 0.5);
            }
        }
    }

    #[test]
    fn empty_and_whitespace_rows_have_valid_logical_bounds() {
        for text in ["", "   ", "Short\n"] {
            let original = layout_text(text, 1.0, Align::Min);
            let height = original.rect.height();
            let aligned = align_galley_rows(original, 80.25, |_| Align::Max);
            assert_eq!(aligned.rect, Rect::from_min_size(pos2(0.0, 0.0), vec2(80.25, height)));
        }
        let original = layout();
        let mut empty = (*original).clone();
        empty.rows.clear();
        empty.rect = Rect::ZERO;
        empty.mesh_bounds = Rect::NOTHING;
        let aligned = align_galley_rows(Arc::new(empty), 80.25, |_| Align::Max);
        assert_eq!(aligned.rect, Rect::from_min_size(pos2(0.0, 0.0), vec2(80.25, 0.0)));
        assert_eq!(aligned.mesh_bounds, Rect::NOTHING);
    }

    #[test]
    fn fractional_scale_zero_width_and_repeated_alignment_keep_absolute_positions() {
        for ppp in [1.25, 1.5] {
            let original = layout_text("Short", ppp, Align::Center);
            for width in [0.0, 163.3] {
                for alignment in [Align::Min, Align::Center, Align::Max] {
                    let direct = align_galley_rows(original.clone(), width, |_| alignment);
                    let first = align_galley_rows(original.clone(), 230.5, |_| Align::Max);
                    let repeated = align_galley_rows(first, width, |_| alignment);
                    assert_eq!(direct.rect.width(), width);
                    for (a, b) in direct.rows.iter().zip(&repeated.rows) {
                        assert_eq!(a.pos, b.pos);
                        let pixel = a.pos.x * ppp;
                        assert!((pixel - pixel.round()).abs() < 1e-4);
                    }
                }
            }
        }
    }

    #[test]
    fn logical_container_and_ink_bounds_remain_distinct() {
        let aligned = align_galley_rows(layout_text("Short", 1.0, Align::Min), 400.0, |_| Align::Min);
        assert_eq!(aligned.rect.width(), 400.0);
        assert!(aligned.mesh_bounds.right() < aligned.rect.right());
        let whitespace = align_galley_rows(layout_text("   ", 1.0, Align::Min), 400.0, |_| Align::Max);
        assert_eq!(whitespace.rect.width(), 400.0);
        assert_eq!(whitespace.mesh_bounds, Rect::NOTHING);
    }

    #[test]
    fn uniquely_owned_galley_reuses_its_allocation() {
        let original = Arc::new((*layout()).clone());
        assert_eq!(Arc::strong_count(&original), 1);
        let before = Arc::as_ptr(&original);
        let aligned = align_galley_rows(original, 400.0, |_| Align::Max);
        assert_eq!(before, Arc::as_ptr(&aligned));
    }
}
