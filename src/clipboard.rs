//! Clipboard history foundation (Phase 5.0): Nott listens for system clipboard
//! changes (`AddClipboardFormatListener` / `WM_CLIPBOARDUPDATE`, no polling) and
//! keeps a small in-memory history of Unicode text and images that later phases
//! can display and restore.
//!
//! Privacy: history lives in memory only. Nothing is written to disk, logs,
//! the network or debug output; clipboard contents are never printed.
//!
//! Ownership: every item is copied into Nott-owned memory while the clipboard is
//! open; no clipboard handle or pointer outlives `CloseClipboard`. Images reuse
//! the media artwork representation (owned BGRA8 premultiplied + fingerprint),
//! but each item owns its own pixels, independent of the media session.

use std::collections::VecDeque;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData,
    GetClipboardOwner, IsClipboardFormatAvailable, OpenClipboard, RemoveClipboardFormatListener,
    SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};

use crate::config::{CLIPBOARD_MAX_ENTRIES, CLIPBOARD_MAX_IMAGE_BYTES, CLIPBOARD_MAX_TEXT_UNITS};
use crate::media::Artwork;

/// Standard clipboard formats (winuser.h).
const CF_UNICODETEXT: u32 = 13;
const CF_DIB: u32 = 8;

/// One clipboard history entry, fully owned by Nott.
#[derive(Clone, PartialEq, Eq)]
pub enum ClipboardItem {
    Text(String),
    /// BGRA8 premultiplied, tightly packed; equality uses dimensions + pixel
    /// fingerprint (computed once, when the image is captured).
    Image(Artwork),
}

/// Never prints contents: kind and size only.
impl std::fmt::Debug for ClipboardItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(t) => write!(f, "Text({} chars)", t.chars().count()),
            Self::Image(a) => write!(f, "Image({}x{})", a.width, a.height),
        }
    }
}

impl ClipboardItem {
    /// Owned image memory of this item (0 for text).
    fn image_bytes(&self) -> usize {
        match self {
            Self::Image(a) => a.pixels.len(),
            Self::Text(_) => 0,
        }
    }
}

/// One-line display preview of `text` (display only: the entry keeps its exact
/// text). Runs of whitespace and control characters, newlines included, become
/// one space; at most `max` characters, ending in an ellipsis when cut.
pub fn preview(text: &str, max: usize) -> String {
    let mut out = String::new();
    let (mut count, mut gap) = (0, false);
    for ch in text.chars() {
        if ch.is_whitespace() || ch.is_control() {
            gap = count > 0;
            continue;
        }
        if count + usize::from(gap) >= max {
            out.push('\u{2026}');
            break;
        }
        if gap {
            out.push(' ');
            count += 1;
            gap = false;
        }
        out.push(ch);
        count += 1;
    }
    out
}

/// Why an item was not stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejected {
    Empty,
    TextTooLong,
    ImageTooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddOutcome {
    Added,
    /// Identical to the newest entry: nothing stored.
    Duplicate,
    Rejected(Rejected),
}

/// Recent clipboard items, newest first, bounded by entry count and total image
/// memory (oldest evicted first). Owned by `WindowState`.
#[derive(Debug, Default)]
pub struct ClipboardHistory {
    items: VecDeque<ClipboardItem>,
}

impl ClipboardHistory {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Newest first.
    pub fn items(&self) -> impl Iterator<Item = &ClipboardItem> {
        self.items.iter()
    }

    pub fn get(&self, index: usize) -> Option<&ClipboardItem> {
        self.items.get(index)
    }

    pub fn image_bytes(&self) -> usize {
        self.items.iter().map(ClipboardItem::image_bytes).sum()
    }

    /// Stores a newly copied item at the front.
    pub fn add(&mut self, item: ClipboardItem) -> AddOutcome {
        if let Err(why) = check_limits(&item) {
            return AddOutcome::Rejected(why);
        }
        if self.items.front() == Some(&item) {
            return AddOutcome::Duplicate;
        }
        self.items.push_front(item);
        while self.items.len() > CLIPBOARD_MAX_ENTRIES
            || self.image_bytes() > CLIPBOARD_MAX_IMAGE_BYTES
        {
            self.items.pop_back();
        }
        AddOutcome::Added
    }

    /// Forgets one entry (its row's trash button).
    pub fn remove(&mut self, index: usize) -> Option<ClipboardItem> {
        self.items.remove(index)
    }

    /// Forgets every entry (and its image memory). The system clipboard is
    /// untouched.
    pub fn clear(&mut self) {
        self.items = VecDeque::new();
    }

    /// Restoring an entry makes it the newest (it is what the clipboard now
    /// holds), without duplicating it. Returns the entry to place on the system
    /// clipboard.
    pub fn promote(&mut self, index: usize) -> Option<&ClipboardItem> {
        let item = self.items.remove(index)?;
        self.items.push_front(item);
        self.items.front()
    }
}

fn check_limits(item: &ClipboardItem) -> Result<(), Rejected> {
    match item {
        ClipboardItem::Text(t) if t.is_empty() => Err(Rejected::Empty),
        ClipboardItem::Text(t) if t.encode_utf16().count() > CLIPBOARD_MAX_TEXT_UNITS => {
            Err(Rejected::TextTooLong)
        }
        ClipboardItem::Image(a) if a.pixels.len() > CLIPBOARD_MAX_IMAGE_BYTES => {
            Err(Rejected::ImageTooLarge)
        }
        _ => Ok(()),
    }
}

// ---- Native clipboard ---------------------------------------------------------

/// Why a clipboard read/write did not happen. Every case is non-fatal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardError {
    /// Another application has the clipboard open (no retry, no spinning).
    Unavailable,
    /// Nothing Nott stores (no text or bitmap).
    Unsupported,
    /// Content over the configured limits (rejected before copying).
    TooLarge,
    /// Malformed or unsupported data.
    Invalid,
    /// Allocation or `SetClipboardData` failed.
    WriteFailed,
}

/// Starts clipboard change notifications (`WM_CLIPBOARDUPDATE`) for `hwnd`.
pub fn start_listening(hwnd: HWND) -> bool {
    unsafe { AddClipboardFormatListener(hwnd).is_ok() }
}

pub fn stop_listening(hwnd: HWND) {
    unsafe {
        let _ = RemoveClipboardFormatListener(hwnd);
    }
}

/// True when the clipboard's current content was put there by `hwnd` (Nott
/// restoring a history item), so the resulting update is not a new copy.
pub fn owned_by(hwnd: HWND) -> bool {
    unsafe { GetClipboardOwner() }.is_ok_and(|owner| owner == hwnd)
}

/// Closes the clipboard when dropped (every early return included).
struct OpenGuard;

impl OpenGuard {
    fn open(owner: Option<HWND>) -> Result<Self, ClipboardError> {
        unsafe { OpenClipboard(owner) }
            .map(|()| Self)
            .map_err(|_| ClipboardError::Unavailable)
    }
}

impl Drop for OpenGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

/// Borrowed view of a clipboard memory block while it is locked.
fn with_locked<R>(handle: HANDLE, f: impl FnOnce(&[u8]) -> R) -> Result<R, ClipboardError> {
    let hglobal = HGLOBAL(handle.0);
    unsafe {
        let ptr = GlobalLock(hglobal) as *const u8;
        if ptr.is_null() {
            return Err(ClipboardError::Invalid);
        }
        let bytes = std::slice::from_raw_parts(ptr, GlobalSize(hglobal));
        let result = f(bytes);
        let _ = GlobalUnlock(hglobal);
        Ok(result)
    }
}

/// Reads the current clipboard into an owned item: Unicode text if present,
/// otherwise a bitmap (CF_DIB, which Windows synthesizes from the other bitmap
/// formats). The clipboard is closed before this returns.
pub fn read(owner: HWND) -> Result<ClipboardItem, ClipboardError> {
    let _open = OpenGuard::open(Some(owner))?;
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT).is_ok() {
            let handle = GetClipboardData(CF_UNICODETEXT).map_err(|_| ClipboardError::Invalid)?;
            return with_locked(handle, |bytes| {
                let units: Vec<u16> = bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes(*c))
                    .take_while(|&u| u != 0)
                    .take(CLIPBOARD_MAX_TEXT_UNITS + 1)
                    .collect();
                text_from_utf16(&units)
            })?
            .map(ClipboardItem::Text);
        }
        if IsClipboardFormatAvailable(CF_DIB).is_ok() {
            let handle = GetClipboardData(CF_DIB).map_err(|_| ClipboardError::Invalid)?;
            return with_locked(handle, dib_to_artwork)?.map(ClipboardItem::Image);
        }
    }
    Err(ClipboardError::Unsupported)
}

/// Places an item on the system clipboard (Nott becomes the owner). On success
/// the memory handle belongs to the system and is not freed here; on failure
/// it is freed. The clipboard is closed before this returns.
#[allow(dead_code)]
pub fn write(owner: HWND, item: &ClipboardItem) -> Result<(), ClipboardError> {
    let (format, bytes) = match item {
        ClipboardItem::Text(t) => {
            let mut bytes: Vec<u8> = t.encode_utf16().flat_map(u16::to_le_bytes).collect();
            bytes.extend([0, 0]);
            (CF_UNICODETEXT, bytes)
        }
        ClipboardItem::Image(a) => (CF_DIB, artwork_to_dib(a)),
    };
    let _open = OpenGuard::open(Some(owner))?;
    unsafe {
        EmptyClipboard().map_err(|_| ClipboardError::WriteFailed)?;
        let hglobal =
            GlobalAlloc(GMEM_MOVEABLE, bytes.len()).map_err(|_| ClipboardError::WriteFailed)?;
        let dst = GlobalLock(hglobal) as *mut u8;
        if dst.is_null() {
            let _ = GlobalFree(Some(hglobal));
            return Err(ClipboardError::WriteFailed);
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
        let _ = GlobalUnlock(hglobal);
        if SetClipboardData(format, Some(HANDLE(hglobal.0))).is_err() {
            // Ownership was not transferred: the memory is still ours
            let _ = GlobalFree(Some(hglobal));
            return Err(ClipboardError::WriteFailed);
        }
    }
    Ok(())
}

// ---- Conversions (pure, testable) -------------------------------------------

/// Owned text from UTF-16 units (already cut at the terminator). Rejects empty
/// or over-limit text without keeping it.
fn text_from_utf16(units: &[u16]) -> Result<String, ClipboardError> {
    if units.is_empty() {
        return Err(ClipboardError::Unsupported);
    }
    if units.len() > CLIPBOARD_MAX_TEXT_UNITS {
        return Err(ClipboardError::TooLarge);
    }
    Ok(String::from_utf16_lossy(units))
}

const BI_RGB: u32 = 0;
const BI_BITFIELDS: u32 = 3;

/// Packed DIB (BITMAPINFOHEADER or later + optional masks/palette + pixels) to
/// owned premultiplied BGRA. Supports 24 and 32 bpp, bottom-up and top-down;
/// 32 bpp with no alpha at all is treated as opaque. The size limit is checked
/// before any pixel is copied.
fn dib_to_artwork(dib: &[u8]) -> Result<Artwork, ClipboardError> {
    let u32_at = |o: usize| {
        dib.get(o..o + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or(ClipboardError::Invalid)
    };
    let header_size = u32_at(0)? as usize;
    if header_size < 40 {
        return Err(ClipboardError::Invalid);
    }
    let width = u32_at(4)? as i32;
    let height = u32_at(8)? as i32;
    let bit_count = u16::from_le_bytes([dib[14], dib[15]]);
    let compression = u32_at(16)?;
    let colors_used = u32_at(32)? as usize;
    if width <= 0 || height == 0 || !(bit_count == 24 || bit_count == 32) {
        return Err(ClipboardError::Invalid);
    }
    let (w, h) = (width as usize, height.unsigned_abs() as usize);
    let out_bytes = w
        .checked_mul(h)
        .and_then(|n| n.checked_mul(4))
        .ok_or(ClipboardError::TooLarge)?;
    if out_bytes > CLIPBOARD_MAX_IMAGE_BYTES {
        return Err(ClipboardError::TooLarge);
    }
    match (compression, bit_count) {
        (BI_RGB, _) => {}
        (BI_BITFIELDS, 32) => {
            // Masks follow a 40-byte header, or live inside V4/V5 headers
            let masks_at = 40;
            let (r, g, b) = (
                u32_at(masks_at)?,
                u32_at(masks_at + 4)?,
                u32_at(masks_at + 8)?,
            );
            if (r, g, b) != (0x00FF_0000, 0x0000_FF00, 0x0000_00FF) {
                return Err(ClipboardError::Invalid);
            }
        }
        _ => return Err(ClipboardError::Invalid),
    }
    let masks = if compression == BI_BITFIELDS && header_size == 40 {
        12
    } else {
        0
    };
    let offset = header_size + masks + colors_used * 4;
    let bpp = bit_count as usize / 8;
    let stride = (w * bit_count as usize).div_ceil(32) * 4;
    let needed = stride.checked_mul(h).and_then(|n| n.checked_add(offset));
    if needed.is_none_or(|n| n > dib.len()) {
        return Err(ClipboardError::Invalid);
    }
    let rows = &dib[offset..];
    let has_alpha =
        bit_count == 32 && (0..h).any(|y| (0..w).any(|x| rows[y * stride + x * 4 + 3] != 0));
    let mut out = vec![0u8; out_bytes];
    for y in 0..h {
        // Bottom-up DIBs store the last row first
        let src_row = if height > 0 { h - 1 - y } else { y };
        let src = &rows[src_row * stride..src_row * stride + w * bpp];
        for x in 0..w {
            let p = &src[x * bpp..x * bpp + bpp];
            let a = if has_alpha { p[3] } else { 255 };
            let pm = |c: u8| ((u32::from(c) * u32::from(a) + 127) / 255) as u8;
            out[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[
                pm(p[0]),
                pm(p[1]),
                pm(p[2]),
                a,
            ]);
        }
    }
    Artwork::new(w as u32, h as u32, out).ok_or(ClipboardError::Invalid)
}

/// Owned image to a packed 32 bpp bottom-up CF_DIB (straight alpha, as DIB
/// consumers expect).
pub(crate) fn artwork_to_dib(a: &Artwork) -> Vec<u8> {
    let (w, h) = (a.width as usize, a.height as usize);
    let mut dib = Vec::with_capacity(40 + w * h * 4);
    for v in [40u32, a.width, a.height] {
        dib.extend(v.to_le_bytes());
    }
    dib.extend(1u16.to_le_bytes()); // planes
    dib.extend(32u16.to_le_bytes()); // bit count
    dib.extend(BI_RGB.to_le_bytes());
    dib.extend(((w * h * 4) as u32).to_le_bytes()); // image size
    dib.extend([0u8; 16]); // resolution, colours used/important
    for y in (0..h).rev() {
        for x in 0..w {
            let p = &a.pixels[(y * w + x) * 4..(y * w + x) * 4 + 4];
            let alpha = p[3];
            let un = |c: u8| {
                if alpha == 0 {
                    0
                } else {
                    ((u32::from(c) * 255 + u32::from(alpha) / 2) / u32::from(alpha)).min(255) as u8
                }
            };
            dib.extend([un(p[0]), un(p[1]), un(p[2]), alpha]);
        }
    }
    dib
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> ClipboardItem {
        ClipboardItem::Text(s.to_string())
    }

    /// Opaque w x h image filled with one colour.
    fn image(w: u32, h: u32, shade: u8) -> ClipboardItem {
        let px: Vec<u8> = (0..w * h)
            .flat_map(|_| [shade, shade / 2, 255 - shade, 255])
            .collect();
        ClipboardItem::Image(Artwork::new(w, h, px).unwrap())
    }

    #[test]
    fn test_preview_normalizes_whitespace_and_newlines() {
        assert_eq!(preview("  hello\r\n\n\tworld  \n", 50), "hello world");
        assert_eq!(preview("a\u{0}b\u{7}\u{1b}c", 50), "a b c");
        assert_eq!(preview("\n\n\t ", 50), "");
        assert_eq!(preview("line one\nline two", 50), "line one line two");
    }

    #[test]
    fn test_preview_truncates_long_text() {
        let long = "word ".repeat(100_000);
        let p = preview(&long, 160);
        assert!(p.ends_with('\u{2026}'));
        assert!(p.chars().count() <= 161, "bounded: max + ellipsis");
        assert!(!p.contains('\n'));
        assert_eq!(preview("short", 160), "short", "no ellipsis when it fits");
        assert_eq!(preview("abcdef", 3), "abc\u{2026}");
    }

    #[test]
    fn test_remove_one_entry_keeps_order() {
        let mut h = ClipboardHistory::default();
        for t in ["a", "b", "c"] {
            h.add(text(t));
        }
        assert_eq!(h.remove(1), Some(text("b")));
        assert_eq!(
            h.items().cloned().collect::<Vec<_>>(),
            [text("c"), text("a")]
        );
        assert_eq!(h.remove(5), None, "out of range is a no-op");
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn test_clear_releases_history_only() {
        let mut h = ClipboardHistory::default();
        h.add(text("a"));
        h.add(image(64, 64, 1));
        assert!(h.image_bytes() > 0);
        h.clear();
        assert!(h.is_empty());
        assert_eq!(h.image_bytes(), 0, "image memory released");
        assert_eq!(h.get(0), None);
        // Still a valid history afterwards
        assert_eq!(h.add(text("b")), AddOutcome::Added);
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn test_empty_history() {
        let h = ClipboardHistory::default();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
        assert_eq!(h.image_bytes(), 0);
        assert!(h.get(0).is_none());
    }

    #[test]
    fn test_add_text_and_image_newest_first() {
        let mut h = ClipboardHistory::default();
        assert_eq!(h.add(text("first")), AddOutcome::Added);
        assert_eq!(h.add(image(4, 4, 10)), AddOutcome::Added);
        assert_eq!(h.add(text("third")), AddOutcome::Added);
        let kinds: Vec<String> = h.items().map(|i| format!("{i:?}")).collect();
        assert_eq!(kinds, ["Text(5 chars)", "Image(4x4)", "Text(5 chars)"]);
        assert_eq!(h.get(0), Some(&text("third")));
        assert_eq!(h.get(2), Some(&text("first")));
        assert_eq!(h.image_bytes(), 4 * 4 * 4);
    }

    #[test]
    fn test_max_entries_evicts_oldest() {
        let mut h = ClipboardHistory::default();
        for i in 0..CLIPBOARD_MAX_ENTRIES + 5 {
            h.add(text(&format!("item {i}")));
        }
        assert_eq!(h.len(), CLIPBOARD_MAX_ENTRIES);
        assert_eq!(
            h.get(0),
            Some(&text(&format!("item {}", CLIPBOARD_MAX_ENTRIES + 4)))
        );
        assert_eq!(
            h.get(CLIPBOARD_MAX_ENTRIES - 1),
            Some(&text("item 5")),
            "oldest gone"
        );
    }

    #[test]
    fn test_image_memory_budget_evicts_oldest() {
        let mut h = ClipboardHistory::default();
        // 1000 x 1000 x 4 = 4 MB each: the third pushes past 10 MB
        h.add(text("keep me?"));
        for shade in [1, 2, 3] {
            assert_eq!(h.add(image(1000, 1000, shade)), AddOutcome::Added);
        }
        assert!(h.image_bytes() <= CLIPBOARD_MAX_IMAGE_BYTES);
        assert_eq!(h.image_bytes(), 8_000_000, "two images remain");
        assert_eq!(h.get(0), Some(&image(1000, 1000, 3)));
        assert_eq!(h.get(1), Some(&image(1000, 1000, 2)));
        assert_eq!(h.len(), 2, "older entries evicted oldest-first");
    }

    #[test]
    fn test_oversized_items_rejected() {
        let mut h = ClipboardHistory::default();
        // 1700 x 1700 x 4 = 11.56 MB > 10 MiB
        assert_eq!(
            h.add(image(1700, 1700, 7)),
            AddOutcome::Rejected(Rejected::ImageTooLarge)
        );
        let long = "x".repeat(CLIPBOARD_MAX_TEXT_UNITS + 1);
        assert_eq!(
            h.add(text(&long)),
            AddOutcome::Rejected(Rejected::TextTooLong)
        );
        assert_eq!(h.add(text("")), AddOutcome::Rejected(Rejected::Empty));
        assert!(h.is_empty(), "nothing stored");
        let fits = "x".repeat(CLIPBOARD_MAX_TEXT_UNITS);
        assert_eq!(h.add(text(&fits)), AddOutcome::Added);
    }

    #[test]
    fn test_consecutive_duplicates_and_distinct_items() {
        let mut h = ClipboardHistory::default();
        assert_eq!(h.add(text("same")), AddOutcome::Added);
        assert_eq!(h.add(text("same")), AddOutcome::Duplicate);
        assert_eq!(h.add(text("other")), AddOutcome::Added);
        assert_eq!(h.add(image(8, 8, 50)), AddOutcome::Added);
        assert_eq!(h.add(image(8, 8, 50)), AddOutcome::Duplicate, "same pixels");
        assert_eq!(
            h.add(image(8, 8, 51)),
            AddOutcome::Added,
            "different pixels"
        );
        assert_eq!(
            h.add(image(4, 16, 51)),
            AddOutcome::Added,
            "different shape"
        );
        // Only *consecutive* duplicates are suppressed
        assert_eq!(h.add(text("same")), AddOutcome::Added);
        assert_eq!(h.len(), 6);
    }

    #[test]
    fn test_items_are_owned_and_survive_source_changes() {
        let mut h = ClipboardHistory::default();
        let mut source = String::from("copied text");
        h.add(ClipboardItem::Text(source.clone()));
        source.clear(); // the "other application" changes its buffer
        let mut pixels = vec![9u8; 2 * 2 * 4];
        h.add(ClipboardItem::Image(
            Artwork::new(2, 2, pixels.clone()).unwrap(),
        ));
        pixels.fill(0);
        assert_eq!(h.get(1), Some(&text("copied text")));
        let ClipboardItem::Image(a) = h.get(0).unwrap() else {
            panic!("image")
        };
        assert!(a.pixels.iter().all(|&b| b == 9), "owned copy unchanged");
    }

    #[test]
    fn test_restore_promotes_without_duplicating() {
        let mut h = ClipboardHistory::default();
        for t in ["a", "b", "c"] {
            h.add(text(t));
        }
        // Restore "a" (oldest): it becomes the newest, still one copy
        assert_eq!(h.promote(2), Some(&text("a")));
        assert_eq!(h.len(), 3);
        assert_eq!(h.get(0), Some(&text("a")));
        // The clipboard update Nott's own restore triggers is a no-op
        assert_eq!(h.add(text("a")), AddOutcome::Duplicate);
        assert_eq!(h.len(), 3);
        assert!(h.promote(9).is_none());
    }

    #[test]
    fn test_debug_output_never_contains_contents() {
        let secret = "hunter2-password";
        let shown = format!("{:?}", text(secret));
        assert!(!shown.contains(secret));
        let mut h = ClipboardHistory::default();
        h.add(text(secret));
        assert!(!format!("{h:?}").contains(secret));
    }

    #[test]
    fn test_text_conversion_and_limits() {
        let units: Vec<u16> = "héllo 👋".encode_utf16().collect();
        assert_eq!(text_from_utf16(&units), Ok("héllo 👋".to_string()));
        assert_eq!(text_from_utf16(&[]), Err(ClipboardError::Unsupported));
        let too_long = vec![b'x' as u16; CLIPBOARD_MAX_TEXT_UNITS + 1];
        assert_eq!(text_from_utf16(&too_long), Err(ClipboardError::TooLarge));
    }

    /// Minimal 24 bpp bottom-up DIB: 2x2, rows padded to 4 bytes.
    fn dib24() -> Vec<u8> {
        let mut d = Vec::new();
        for v in [40u32, 2, 2] {
            d.extend(v.to_le_bytes());
        }
        d.extend(1u16.to_le_bytes());
        d.extend(24u16.to_le_bytes());
        d.extend([0u8; 24]);
        // bottom row: blue, green; top row: red, white
        d.extend([255, 0, 0, 0, 255, 0, 0, 0]);
        d.extend([0, 0, 255, 255, 255, 255, 0, 0]);
        d
    }

    #[test]
    fn test_dib_decode_orientation_and_alpha() {
        let a = dib_to_artwork(&dib24()).unwrap();
        assert_eq!((a.width, a.height), (2, 2));
        // Top-left is the red pixel (bottom-up flipped), opaque
        assert_eq!(&a.pixels[0..4], &[0, 0, 255, 255]);
        assert_eq!(&a.pixels[12..16], &[0, 255, 0, 255], "bottom-right green");
        // Round trip through Nott's own CF_DIB keeps the pixels
        let back = dib_to_artwork(&artwork_to_dib(&a)).unwrap();
        assert_eq!(back, a);
    }

    #[test]
    fn test_dib_alpha_round_trip_and_malformed_input() {
        // Half-transparent pixel survives premultiply -> DIB -> premultiply
        let a = Artwork::new(1, 1, vec![64, 32, 16, 128]).unwrap();
        let back = dib_to_artwork(&artwork_to_dib(&a)).unwrap();
        for (x, y) in back.pixels.iter().zip(a.pixels.iter()) {
            assert!((*x as i32 - *y as i32).abs() <= 1);
        }
        assert_eq!(dib_to_artwork(&[]), Err(ClipboardError::Invalid));
        assert_eq!(
            dib_to_artwork(&dib24()[..50]),
            Err(ClipboardError::Invalid),
            "truncated"
        );
        let mut bad_bpp = dib24();
        bad_bpp[14] = 8;
        assert_eq!(dib_to_artwork(&bad_bpp), Err(ClipboardError::Invalid));
        // Huge declared size is rejected before allocating
        let mut huge = dib24();
        huge[4..8].copy_from_slice(&5000u32.to_le_bytes());
        huge[8..12].copy_from_slice(&5000u32.to_le_bytes());
        assert_eq!(dib_to_artwork(&huge), Err(ClipboardError::TooLarge));
    }

    /// Real clipboard (run explicitly: `cargo test clipboard_live -- --ignored`).
    /// Overwrites the user's clipboard.
    #[test]
    #[ignore]
    fn clipboard_live_write_read_and_busy_failure() {
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
        };
        // A message-only window owns the clipboard (SetClipboardData needs one)
        let owner = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                windows::core::w!("STATIC"),
                windows::core::w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
        }
        .expect("owner window");
        let item = text("nott clipboard test");
        write(owner, &item).expect("write");
        assert!(owned_by(owner), "Nott's own write is recognisable");
        assert_eq!(read(owner), Ok(item));
        let img = image(3, 2, 77);
        write(owner, &img).expect("write image");
        assert_eq!(read(owner), Ok(img.clone()));
        // Clearing the history leaves the system clipboard as it was
        let mut history = ClipboardHistory::default();
        history.add(img.clone());
        history.clear();
        assert_eq!(read(owner), Ok(img));
        // Another thread holds the clipboard open: read/write fail cleanly
        let (tx, rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let holder = std::thread::spawn(move || {
            let _g = OpenGuard::open(None).expect("open");
            tx.send(()).unwrap();
            let _ = done_rx.recv();
        });
        rx.recv().unwrap();
        assert_eq!(read(owner), Err(ClipboardError::Unavailable));
        assert_eq!(write(owner, &text("x")), Err(ClipboardError::Unavailable));
        done_tx.send(()).unwrap();
        holder.join().unwrap();
        unsafe {
            let _ = DestroyWindow(owner);
        }
    }
}
