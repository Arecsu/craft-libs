//! Drawing text in a font face itself: each face shown becomes an egui font family of its own.
//!
//! Adding a font makes egui rebuild its fonts, so faces are added a few per frame, and once more
//! than [`MAX_REGISTERED`] are loaded they are all taken out again, which keeps memory to the faces
//! on screen. Only the panel's own faces are touched: the host's fonts stay as they are.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::{Catalog, FaceInfo};
use egui::epaint::text::{Blob, FontInsert, FontPriority, InsertFontFamily, VariationCoords};
use egui::{Context, FontData, FontFamily, FontTweak};

const MAX_REGISTERED: usize = 64;
const PER_FRAME: usize = 6;
/// Larger files are not drawn: a whole CJK family can be hundreds of megabytes.
const MAX_FILE_BYTES: u64 = 48 << 20;

/// Whose catalog a face belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    /// The installed fonts.
    Installed,
    /// Files downloaded for a preview.
    Preview,
}

pub enum Shown {
    Ready(FontFamily),
    Loading,
    Unavailable,
}

#[derive(Default)]
pub struct FaceFonts {
    registered: HashMap<String, FontFamily>,
    failed: HashSet<String>,
    budget: usize,
}

fn key(owner: Owner, face: &FaceInfo) -> String {
    let file = match &face.path {
        Some(path) => path.display().to_string(),
        None => face.postscript_name.clone().or_else(|| face.full_name.clone()).unwrap_or_else(|| face.family.clone()),
    };
    let coords: String = face.variations.iter().map(|(tag, v)| format!(" {}={v}", String::from_utf8_lossy(tag))).collect();
    format!("face:{owner:?}:{file}#{}{coords}", face.index)
}

impl FaceFonts {
    pub fn begin_frame(&mut self, ctx: &Context) {
        self.budget = PER_FRAME;
        if self.registered.len() > MAX_REGISTERED {
            let mut definitions = ctx.fonts(|f| f.definitions().clone());
            definitions.font_data.retain(|name, _| !self.registered.contains_key(name));
            definitions.families.retain(|family, _| !self.registered.values().any(|f| f == family));
            ctx.set_fonts(definitions);
            self.registered.clear();
        }
    }

    /// The family that draws `face`, once egui has loaded it.
    pub fn family(&mut self, ctx: &Context, catalog: &Catalog, owner: Owner, face: &FaceInfo) -> Shown {
        let key = key(owner, face);
        if self.failed.contains(&key) {
            return Shown::Unavailable;
        }
        if let Some(family) = self.registered.get(&key) {
            let loaded = ctx.fonts(|f| f.definitions().families.contains_key(family));
            if loaded {
                return Shown::Ready(family.clone());
            }
            ctx.request_repaint();
            return Shown::Loading;
        }
        if self.budget == 0 {
            ctx.request_repaint();
            return Shown::Loading;
        }
        self.budget -= 1;
        let too_large = face.path.as_ref().and_then(|p| std::fs::metadata(p).ok()).is_some_and(|m| m.len() > MAX_FILE_BYTES);
        let data = if too_large { None } else { catalog.load(face).ok() };
        let Some(data) = data else {
            self.failed.insert(key);
            return Shown::Unavailable;
        };
        let blob: Blob = Arc::new(data.bytes);
        let tweak = FontTweak { coords: VariationCoords::new(face.variations.iter().map(|(tag, v)| (*tag, *v))), ..Default::default() };
        let family = FontFamily::Name(key.as_str().into());
        let insert = InsertFontFamily { family: family.clone(), priority: FontPriority::Highest };
        ctx.add_font(FontInsert::new(&key, FontData::from_blob(blob, data.index).tweak(tweak), vec![insert]));
        self.registered.insert(key, family);
        ctx.request_repaint();
        Shown::Loading
    }
}
