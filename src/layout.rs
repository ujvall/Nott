#![allow(dead_code)]

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

use crate::config::{
    BASE_MEDIA_ARTIST_HEIGHT, BASE_MEDIA_ARTWORK_GAP, BASE_MEDIA_ARTWORK_INSET,
    BASE_MEDIA_ARTWORK_SIZE, BASE_MEDIA_CLOCK_COLUMN_WIDTH, BASE_MEDIA_COLUMN_GAP,
    BASE_MEDIA_CONTROL_GAP, BASE_MEDIA_CONTROL_HEIGHT, BASE_MEDIA_CONTROL_VISUAL_SIZE,
    BASE_MEDIA_CONTROL_WIDTH, BASE_MEDIA_CONTROLS_SPACING, BASE_MEDIA_ICON_SIZE,
    BASE_MEDIA_SOURCE_HEIGHT, BASE_MEDIA_TIMELINE_LEAD, BASE_MEDIA_TITLE_HEIGHT,
    BASE_MEDIA_VISUALIZER_GAP, BASE_VISUALIZER_BAR_GAP, BASE_VISUALIZER_BAR_WIDTH,
    BASE_VISUALIZER_HEIGHT, NotchDimensions, NotchState, SKIP_GLYPH_HALF_WIDTH,
};
use crate::media::MediaControl;
use crate::media::VISUALIZER_BARS;

/// Floating-point axis-aligned rectangle for layout computation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectF {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl RectF {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    #[inline]
    pub fn width(&self) -> f32 {
        (self.right - self.left).max(0.0)
    }

    #[inline]
    pub fn height(&self) -> f32 {
        (self.bottom - self.top).max(0.0)
    }

    #[inline]
    pub fn to_d2d_rect(self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.left,
            top: self.top,
            right: self.right,
            bottom: self.bottom,
        }
    }

    #[inline]
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.left && px <= self.right && py >= self.top && py <= self.bottom
    }
}

/// Resolved layout components for the collapsed notch state
#[derive(Debug, Clone, PartialEq)]
pub struct CollapsedLayout {
    /// Safe boundary inside the collapsed notch respecting padding and hardware curvature
    pub content_bounds: RectF,
    /// Centered clock text bounds for live Windows time
    pub clock_bounds: RectF,
    /// Playback visualizer, right end of the content area (inside the notch body)
    pub visualizer_bounds: RectF,
}

/// Whole-pixel (bar width, bar gap, full height) of the visualizer at `scale`.
pub fn visualizer_metrics(scale: f32) -> (f32, f32, f32) {
    let px = |v: f32| (v * scale).round().max(1.0);
    (
        px(BASE_VISUALIZER_BAR_WIDTH),
        px(BASE_VISUALIZER_BAR_GAP),
        px(BASE_VISUALIZER_HEIGHT),
    )
}

/// Visualizer slot whose right edge is `right`, vertically centered on `cy`.
fn visualizer_rect(right: f32, cy: f32, scale: f32) -> RectF {
    let (bar, gap, h) = visualizer_metrics(scale);
    let n = VISUALIZER_BARS as f32;
    let w = n * bar + (n - 1.0) * gap;
    let top = (cy - h / 2.0).round();
    RectF::new(right - w, top, right, top + h)
}

/// Resolved layout components for the expanded notch state: live time and date
#[derive(Debug, Clone, PartialEq)]
pub struct ExpandedLayout {
    /// Safe boundary inside the notch respecting padding and hardware curvature
    pub content_bounds: RectF,
    /// Centered live time bounds (e.g. "12:47 AM")
    pub time_bounds: RectF,
    /// Centered live date bounds (e.g. "Monday, October 6")
    pub date_bounds: RectF,
}

/// Overall layout resolved from NotchDimensions and NotchState
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedLayout {
    Collapsed {
        bounds: RectF,
        components: CollapsedLayout,
    },
    Expanded {
        bounds: RectF,
        components: ExpandedLayout,
    },
}

impl ResolvedLayout {
    /// Returns the outer bounding rectangle of the notch surface
    pub fn bounds(&self) -> RectF {
        match self {
            Self::Collapsed { bounds, .. } => *bounds,
            Self::Expanded { bounds, .. } => *bounds,
        }
    }
}

/// Expanded composition when a media session is available. Visual rectangles lie
/// inside `ExpandedLayout::content_bounds`; control *hit* rectangles may extend into
/// the notch's bottom padding (still inside the notch contour). Nothing here
/// changes the notch itself.
///
/// ```text
/// [artwork] [title                     ]   [time]
///           [artist — album            ]   [date]
///           [source app                ]
///              [prev] [play/pause] [next]
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaLayout {
    /// Square artwork slot (None when the track has no artwork; text then starts
    /// at the content edge)
    pub artwork_bounds: Option<RectF>,
    /// Primary line (single line, ellipsis-trimmed)
    pub title_bounds: RectF,
    /// Artist / album line
    pub artist_bounds: RectF,
    /// Source application line (smallest hierarchy)
    pub source_bounds: RectF,
    /// Secondary live time / short date, right column
    pub time_bounds: RectF,
    pub date_bounds: RectF,
    /// Hit areas of the transport controls, left-aligned with the text column
    pub previous_bounds: RectF,
    pub play_pause_bounds: RectF,
    pub next_bounds: RectF,
    /// Diameter of each control's visual backdrop (top-aligned in its hit area)
    pub control_visual_size: f32,
    /// Playback visualizer at the end of the title row
    pub visualizer_bounds: RectF,
    /// Inline scrubber strip (elapsed label, track, remaining label) in the
    /// controls row, right of the controls; may be too narrow to draw
    pub timeline_bounds: RectF,
}

impl MediaLayout {
    pub fn control_bounds(&self, control: MediaControl) -> RectF {
        match control {
            MediaControl::Previous => self.previous_bounds,
            MediaControl::PlayPause => self.play_pause_bounds,
            MediaControl::Next => self.next_bounds,
        }
    }

    /// Square visual area of a control (backdrop/icon), top-aligned in its hit area
    /// so it stays inside the content bounds.
    pub fn control_visual_bounds(&self, control: MediaControl) -> RectF {
        let hit = self.control_bounds(control);
        let d = self.control_visual_size;
        let cx = (hit.left + hit.right) / 2.0;
        RectF::new(cx - d / 2.0, hit.top, cx + d / 2.0, hit.top + d)
    }

    /// Returns the transport control under the client point, if any.
    pub fn control_at(&self, px: f32, py: f32) -> Option<MediaControl> {
        MediaControl::ALL
            .into_iter()
            .find(|c| self.control_bounds(*c).contains(px, py))
    }
}

/// Resolves the media composition inside the expanded content area. Returns `None`
/// for collapsed dimensions. Derived from the current layout system (content bounds)
/// and DPI-scaled DIP tokens; during animation the stack anchors at the content top
/// and is clipped by the interpolated content bounds.
/// What the media composition has to show; drives artwork slot and vertical balance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaShape {
    pub artwork: bool,
    /// Lines under the title (artist/album, source): 0..=2.
    pub secondary_lines: u32,
}

impl MediaShape {
    pub const FULL: Self = Self {
        artwork: true,
        secondary_lines: 2,
    };
}

pub fn resolve_media_layout(
    dimensions: &NotchDimensions,
    shape: MediaShape,
) -> Option<MediaLayout> {
    let has_artwork = shape.artwork;
    let ResolvedLayout::Expanded { components, .. } = resolve_layout(dimensions) else {
        return None;
    };
    let c = components.content_bounds;
    let s = dimensions.scale;
    let px = |v: f32| (v * s).round();

    let title_h = px(BASE_MEDIA_TITLE_HEIGHT);
    let artist_h = px(BASE_MEDIA_ARTIST_HEIGHT);
    let source_h = px(BASE_MEDIA_SOURCE_HEIGHT);
    let text_h =
        px(BASE_MEDIA_TITLE_HEIGHT) + px(BASE_MEDIA_ARTIST_HEIGHT) + px(BASE_MEDIA_SOURCE_HEIGHT);
    // Shrinks (by rounding slack only) so the control row never leaves the content area
    let visual = px(BASE_MEDIA_CONTROL_VISUAL_SIZE).min((c.height() - text_h).max(0.0));
    let control_w = px(BASE_MEDIA_CONTROL_WIDTH);
    // The hit area gives up whatever the visual row lost, so it never reaches
    // further into the rounded bottom corners.
    let control_h =
        (px(BASE_MEDIA_CONTROL_HEIGHT) - (px(BASE_MEDIA_CONTROL_VISUAL_SIZE) - visual)).max(visual);

    // The text/controls gap absorbs per-DPI rounding so the visual stack never
    // exceeds the content height.
    let fixed_h = title_h + artist_h + source_h + visual;
    let full_gap = px(BASE_MEDIA_CONTROLS_SPACING).min((c.height() - fixed_h).max(0.0));
    let full_top = (c.top + (c.height() - fixed_h - full_gap) / 2.0).max(c.top);
    // Fewer secondary lines: the text + controls block stays together and is
    // re-centered vertically (no empty row above the controls). The clock column
    // keeps the full layout's rows so environmental info never moves.
    let missing_h = match shape.secondary_lines {
        0 => artist_h + source_h,
        1 => source_h,
        _ => 0.0,
    };
    // Breathing room above the controls comes from the rows actually shown.
    let shown_h = fixed_h - missing_h;
    let gap = px(BASE_MEDIA_CONTROLS_SPACING).min((c.height() - shown_h).max(0.0));
    let top = (c.top + ((c.height() - shown_h - gap) / 2.0).round()).max(c.top);

    // The artwork sits out at the notch's bottom-left corner: the same inset from
    // the side wall as from the bottom edge, so it nests in the corner.
    let artwork_bounds = has_artwork.then(|| {
        // Whole-pixel cover (and so badge) edges at every scale
        let side = px(BASE_MEDIA_ARTWORK_SIZE).min(c.height()).max(0.0).floor();
        let art_top = (dimensions.notch_height() - px(BASE_MEDIA_ARTWORK_INSET) - side)
            .round()
            .max(c.top.ceil());
        let inset = (dimensions.notch_height() - (art_top + side)).max(0.0);
        let wall = dimensions.shadow_margin_x + dimensions.curvature.top_transition_radius;
        let art_left = (wall + inset).round().min(c.left);
        RectF::new(art_left, art_top, art_left + side, art_top + side)
    });
    let clock_w = px(BASE_MEDIA_CLOCK_COLUMN_WIDTH).min(c.width() / 3.0);
    let clock_left = c.right - clock_w;
    let text_left = artwork_bounds.map_or(c.left, |a| a.right + px(BASE_MEDIA_ARTWORK_GAP));
    let text_right = (clock_left - px(BASE_MEDIA_COLUMN_GAP)).max(text_left);

    let row = |y: f32, h: f32| RectF::new(text_left, y, text_right, y + h);
    // The title gives up the end of its row to the visualizer (always reserved,
    // so the title never reflows on play/pause)
    let visualizer_bounds = visualizer_rect(text_right, top + title_h / 2.0, s);
    let title_bounds = RectF {
        right: (visualizer_bounds.left - px(BASE_MEDIA_VISUALIZER_GAP)).max(text_left),
        ..row(top, title_h)
    };
    let artist_bounds = row(title_bounds.bottom, artist_h);
    let source_bounds = row(artist_bounds.bottom, source_h);

    // The control group shares the text column's left edge: the Previous glyph's
    // left edge sits on the text edge, so metadata and controls read as one block.
    let controls_top = source_bounds.bottom - missing_h + gap;
    let step = control_w + px(BASE_MEDIA_CONTROL_GAP);
    // ...unless that would push the hover backdrop past the content edge (no artwork)
    // and the hit area stays clear of the rounded bottom corner (no-artwork case).
    let first_cx = (text_left + (px(BASE_MEDIA_ICON_SIZE) * SKIP_GLYPH_HALF_WIDTH).round())
        .max(c.left + visual / 2.0)
        .max(c.left + control_w / 2.0);
    let control = |index: f32| {
        let left = first_cx + index * step - control_w / 2.0;
        RectF::new(
            left,
            controls_top,
            left + control_w,
            controls_top + control_h,
        )
    };

    // Clear of Next's backdrop and of its (wider) hit area
    let next_cx = first_cx + 2.0 * step;
    let timeline_left = (next_cx + visual / 2.0 + px(BASE_MEDIA_TIMELINE_LEAD))
        .max(next_cx + control_w / 2.0)
        .min(text_right);
    let timeline_bounds = RectF::new(
        timeline_left,
        controls_top,
        text_right,
        controls_top + visual,
    );

    Some(MediaLayout {
        artwork_bounds,
        title_bounds,
        artist_bounds,
        source_bounds,
        time_bounds: RectF::new(clock_left, full_top, c.right, full_top + title_h),
        date_bounds: RectF::new(
            clock_left,
            full_top + title_h,
            c.right,
            full_top + title_h + artist_h,
        ),
        previous_bounds: control(0.0),
        play_pause_bounds: control(1.0),
        next_bounds: control(2.0),
        control_visual_size: visual,
        visualizer_bounds,
        timeline_bounds,
    })
}

/// Resolves collapsed layout components given NotchDimensions
pub fn resolve_collapsed_layout(dimensions: &NotchDimensions) -> CollapsedLayout {
    let w = dimensions.width as f32;
    let h = dimensions.height as f32;
    let scale = dimensions.scale;
    let r_top = dimensions.curvature.top_transition_radius;
    let pad_h = dimensions.padding_h;
    let pad_v = dimensions.padding_v;

    // Safe content bounds respecting shoulder transition and padding
    let content_left = r_top + pad_h;
    let content_right = (w - r_top - pad_h).max(content_left);
    let content_top = pad_v;
    let content_bottom = (h - pad_v).max(content_top);
    let content_bounds = RectF::new(content_left, content_top, content_right, content_bottom);

    let optical_y = crate::config::BASE_CLOCK_OPTICAL_Y_OFFSET * scale;
    let clock_bounds = RectF::new(
        content_left,
        content_top + optical_y,
        content_right,
        content_bottom + optical_y,
    );

    CollapsedLayout {
        content_bounds,
        clock_bounds,
        visualizer_bounds: visualizer_rect(
            content_right,
            (content_top + content_bottom) / 2.0,
            scale,
        ),
    }
}

/// Resolves complete layout and component boundaries from NotchDimensions
pub fn resolve_layout(dimensions: &NotchDimensions) -> ResolvedLayout {
    let w = dimensions.width as f32;
    let h = dimensions.height as f32;
    let bounds = RectF::new(0.0, 0.0, w, h);

    match dimensions.state {
        NotchState::Collapsed => ResolvedLayout::Collapsed {
            bounds,
            components: resolve_collapsed_layout(dimensions),
        },
        NotchState::Expanded => {
            let scale = dimensions.scale;
            let pad_x = dimensions.shadow_margin_x;
            let notch_w = dimensions.notch_width();
            let notch_h = dimensions.notch_height();
            let r_top = dimensions.curvature.top_transition_radius;
            let pad_h = dimensions.padding_h;
            let pad_v = dimensions.padding_v;

            // Safe content bounds: avoids top shoulder curves, shadow margins, and bottom rounded corners
            let content_left = pad_x + r_top + pad_h;
            let content_right = (pad_x + notch_w - r_top - pad_h).max(content_left);
            let content_top = dimensions.curvature.top_transition_height + pad_v;
            let content_bottom = (notch_h - pad_v).max(content_top);
            let content_bounds =
                RectF::new(content_left, content_top, content_right, content_bottom);

            // Optical layout for stacked live time and date
            let time_height = (crate::config::BASE_EXPANDED_TIME_HEIGHT * scale).round();
            let date_height = (crate::config::BASE_EXPANDED_DATE_HEIGHT * scale).round();
            let spacing = (crate::config::BASE_EXPANDED_CLOCK_SPACING * scale).round();
            let optical_y = (crate::config::BASE_EXPANDED_CLOCK_OPTICAL_Y_OFFSET * scale).round();

            let total_stack_height = time_height + spacing + date_height;
            let available_height = content_bounds.height();
            let stack_top =
                (content_top + (available_height - total_stack_height) / 2.0 + optical_y)
                    .max(content_top);

            let time_top = stack_top;
            let time_bottom = (time_top + time_height).min(content_bottom);
            let time_bounds = RectF::new(content_left, time_top, content_right, time_bottom);

            let date_top = (time_bottom + spacing).min(content_bottom);
            let date_bottom = (date_top + date_height).min(content_bottom);
            let date_bounds = RectF::new(content_left, date_top, content_right, date_bottom);

            ResolvedLayout::Expanded {
                bounds,
                components: ExpandedLayout {
                    content_bounds,
                    time_bounds,
                    date_bounds,
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BASE_EXPANDED_HEIGHT, BASE_EXPANDED_WIDTH, is_point_in_notch_ex};

    #[test]
    fn test_expanded_layout_dimensions_and_containment() {
        for dpi in [96, 120, 137, 144, 168, 192, 288] {
            let dims = NotchDimensions::from_state_and_dpi(NotchState::Expanded, dpi);
            let layout = resolve_layout(&dims);

            match layout {
                ResolvedLayout::Expanded { bounds, components } => {
                    assert_eq!(bounds.width(), dims.width as f32);
                    assert_eq!(bounds.height(), dims.height as f32);

                    // Content bounds inside outer notch bounds
                    assert!(components.content_bounds.left >= 0.0);
                    assert!(components.content_bounds.top >= 0.0);
                    assert!(components.content_bounds.right <= bounds.right);
                    assert!(components.content_bounds.bottom <= bounds.bottom);

                    // Time bounds inside content bounds
                    assert!(components.time_bounds.left >= components.content_bounds.left);
                    assert!(components.time_bounds.right <= components.content_bounds.right);
                    assert!(components.time_bounds.top >= components.content_bounds.top);
                    assert!(components.time_bounds.bottom <= components.content_bounds.bottom);

                    // Date bounds inside content bounds, placed below time bounds
                    assert!(components.date_bounds.left >= components.content_bounds.left);
                    assert!(components.date_bounds.right <= components.content_bounds.right);
                    assert!(components.date_bounds.top >= components.time_bounds.bottom);
                    assert!(components.date_bounds.bottom <= components.content_bounds.bottom);

                    // Horizontal centering check: centers of time and date bounds must equal notch center
                    let center_x = bounds.width() / 2.0;
                    let time_center_x =
                        (components.time_bounds.left + components.time_bounds.right) / 2.0;
                    let date_center_x =
                        (components.date_bounds.left + components.date_bounds.right) / 2.0;
                    assert!(
                        (time_center_x - center_x).abs() <= 1.0,
                        "Time must be horizontally centered at {dpi} DPI"
                    );
                    assert!(
                        (date_center_x - center_x).abs() <= 1.0,
                        "Date must be horizontally centered at {dpi} DPI"
                    );

                    // All four corners of content_bounds must fall inside the AMOLED notch geometry
                    assert!(
                        dims.contains_point(
                            components.content_bounds.left,
                            components.content_bounds.top,
                        ),
                        "Top-left content bound must be inside notch geometry at {dpi} DPI"
                    );
                    assert!(
                        dims.contains_point(
                            components.content_bounds.right,
                            components.content_bounds.top,
                        ),
                        "Top-right content bound must be inside notch geometry at {dpi} DPI"
                    );
                    assert!(
                        dims.contains_point(
                            components.content_bounds.left,
                            components.content_bounds.bottom,
                        ),
                        "Bottom-left content bound must be inside notch geometry at {dpi} DPI"
                    );
                    assert!(
                        dims.contains_point(
                            components.content_bounds.right,
                            components.content_bounds.bottom,
                        ),
                        "Bottom-right content bound must be inside notch geometry at {dpi} DPI"
                    );
                }
                _ => panic!("Expected ResolvedLayout::Expanded"),
            }
        }
    }

    #[test]
    fn test_collapsed_and_expanded_centering_invariance() {
        // Collapsed center must equal expanded center for any monitor width and DPI
        for screen_w in [1024, 1280, 1366, 1440, 1536, 1920, 2560, 3840] {
            for dpi in [96, 120, 144, 168, 192] {
                let dims_c = NotchDimensions::from_state_and_dpi(NotchState::Collapsed, dpi);
                let dims_e = NotchDimensions::from_state_and_dpi(NotchState::Expanded, dpi);

                let center_c = crate::config::calculate_notch_center_x(screen_w, dims_c.width);
                let center_e = crate::config::calculate_notch_center_x(screen_w, dims_e.width);

                // Both centers must equal screen_w / 2 when notch fits on screen
                let ideal_center = screen_w as f32 / 2.0;
                assert!(
                    (center_c - ideal_center).abs() <= 0.5,
                    "Collapsed center mismatch at screen {screen_w}, dpi {dpi}"
                );
                if screen_w >= dims_e.width {
                    assert!(
                        (center_e - ideal_center).abs() <= 0.5,
                        "Expanded center mismatch at screen {screen_w}, dpi {dpi}"
                    );
                    assert!(
                        (center_c - center_e).abs() <= 1.0,
                        "Collapsed and expanded centers must match within rounding at screen {screen_w}, dpi {dpi}"
                    );
                } else {
                    // When display is narrower than expanded notch at high scale factor, clamped safely to 0
                    assert_eq!(crate::config::calculate_notch_x(screen_w, dims_e.width), 0);
                }
            }
        }
    }

    #[test]
    fn test_collapsed_layout_clock_containment_and_centering() {
        for dpi in [96, 120, 137, 144, 168, 192, 288] {
            let dims = NotchDimensions::from_state_and_dpi(NotchState::Collapsed, dpi);
            let layout = resolve_layout(&dims);

            match layout {
                ResolvedLayout::Collapsed { bounds, components } => {
                    assert_eq!(bounds.width(), dims.width as f32);
                    assert_eq!(bounds.height(), dims.height as f32);

                    // Content bounds inside outer notch bounds
                    assert!(components.content_bounds.left >= 0.0);
                    assert!(components.content_bounds.top >= 0.0);
                    assert!(components.content_bounds.right <= bounds.right);
                    assert!(components.content_bounds.bottom <= bounds.bottom);

                    // Clock bounds inside content bounds
                    assert!(components.clock_bounds.left >= components.content_bounds.left);
                    assert!(components.clock_bounds.right <= components.content_bounds.right);

                    // Mathematical horizontal centering: clock center must equal notch center w / 2.0 exactly
                    let clock_cx =
                        (components.clock_bounds.left + components.clock_bounds.right) / 2.0;
                    let notch_cx = dims.width as f32 / 2.0;
                    assert!(
                        (clock_cx - notch_cx).abs() <= 0.001,
                        "Clock horizontal center ({clock_cx}) must equal notch center ({notch_cx}) at {dpi} DPI"
                    );

                    // Vertical centering: clock vertical center matches notch vertical center h / 2.0
                    let clock_cy =
                        (components.clock_bounds.top + components.clock_bounds.bottom) / 2.0;
                    let notch_cy = dims.height as f32 / 2.0;
                    assert!(
                        (clock_cy - notch_cy).abs() <= 0.5,
                        "Clock vertical center ({clock_cy}) must match notch vertical center ({notch_cy}) at {dpi} DPI"
                    );

                    // All four corners of content_bounds must fall inside the AMOLED notch geometry
                    let r_top_x = dims.curvature.top_transition_radius;
                    let r_top_y = dims.curvature.top_transition_height;
                    let r_bottom = dims.curvature.bottom_radius;
                    let w = dims.width as f32;
                    let h = dims.height as f32;

                    assert!(
                        is_point_in_notch_ex(
                            components.content_bounds.left,
                            components.content_bounds.top,
                            w,
                            h,
                            r_top_x,
                            r_top_y,
                            r_bottom
                        ),
                        "Collapsed top-left content bound must be inside notch geometry at {dpi} DPI"
                    );
                    assert!(
                        is_point_in_notch_ex(
                            components.content_bounds.right,
                            components.content_bounds.top,
                            w,
                            h,
                            r_top_x,
                            r_top_y,
                            r_bottom
                        ),
                        "Collapsed top-right content bound must be inside notch geometry at {dpi} DPI"
                    );
                    assert!(
                        is_point_in_notch_ex(
                            components.content_bounds.left,
                            components.content_bounds.bottom,
                            w,
                            h,
                            r_top_x,
                            r_top_y,
                            r_bottom
                        ),
                        "Collapsed bottom-left content bound must be inside notch geometry at {dpi} DPI"
                    );
                    assert!(
                        is_point_in_notch_ex(
                            components.content_bounds.right,
                            components.content_bounds.bottom,
                            w,
                            h,
                            r_top_x,
                            r_top_y,
                            r_bottom
                        ),
                        "Collapsed bottom-right content bound must be inside notch geometry at {dpi} DPI"
                    );
                }
                _ => panic!("Expected ResolvedLayout::Collapsed"),
            }
        }
    }

    #[test]
    fn test_collapsed_state_does_not_expose_expanded_content() {
        let dims = NotchDimensions::from_state_and_dpi(NotchState::Collapsed, 96);
        let layout = resolve_layout(&dims);
        match layout {
            ResolvedLayout::Collapsed { components, .. } => {
                // Collapsed state only has content_bounds and clock_bounds
                assert!(components.content_bounds.width() > 0.0);
                assert!(components.clock_bounds.width() > 0.0);
            }
            _ => panic!("Expected ResolvedLayout::Collapsed"),
        }
    }

    #[test]
    fn test_collapsed_and_expanded_horizontal_center_alignment() {
        for dpi in [96, 120, 144, 192] {
            let dims_c = NotchDimensions::from_state_and_dpi(NotchState::Collapsed, dpi);
            let dims_e = NotchDimensions::from_state_and_dpi(NotchState::Expanded, dpi);

            let layout_c = resolve_layout(&dims_c);
            let layout_e = resolve_layout(&dims_e);

            if let (
                ResolvedLayout::Collapsed {
                    components: comp_c, ..
                },
                ResolvedLayout::Expanded {
                    bounds: bounds_e, ..
                },
            ) = (layout_c, layout_e)
            {
                // Collapsed clock center is exactly width / 2.0
                let clock_cx = (comp_c.clock_bounds.left + comp_c.clock_bounds.right) / 2.0;
                let notch_c_cx = dims_c.width as f32 / 2.0;
                assert!((clock_cx - notch_c_cx).abs() <= 0.001);

                // Both collapsed notch and expanded notch have their symmetry axes at half their width
                assert_eq!(bounds_e.width(), dims_e.width as f32);
            } else {
                panic!("State resolution mismatch");
            }
        }
    }

    #[test]
    fn test_reference_dimensions_at_96_dpi() {
        let dims_e = NotchDimensions::from_state_and_dpi(NotchState::Expanded, 96);
        assert_eq!(dims_e.width, BASE_EXPANDED_WIDTH as i32);
        assert_eq!(dims_e.height, BASE_EXPANDED_HEIGHT as i32);
        assert_eq!(dims_e.width, 600);
        assert_eq!(dims_e.height, 128);
    }

    #[test]
    fn test_expanded_clock_and_date_bounds() {
        let dims = NotchDimensions::from_state_and_dpi(NotchState::Expanded, 96);
        let layout = resolve_layout(&dims);
        match layout {
            ResolvedLayout::Expanded { components, .. } => {
                assert!(components.time_bounds.width() > 0.0);
                assert_eq!(components.time_bounds.height(), 34.0);
                assert!(components.date_bounds.width() > 0.0);
                assert_eq!(components.date_bounds.height(), 16.0);
                assert!(components.date_bounds.top >= components.time_bounds.bottom);
            }
            _ => panic!("Expected ResolvedLayout::Expanded"),
        }
    }

    #[test]
    fn test_expanded_time_and_date_containment_across_all_dpis() {
        for dpi in [96, 120, 137, 144, 168, 192, 288] {
            let dims = NotchDimensions::from_state_and_dpi(NotchState::Expanded, dpi);
            let layout = resolve_layout(&dims);
            match layout {
                ResolvedLayout::Expanded { bounds, components } => {
                    // Content bounds inside outer notch bounds
                    assert!(components.content_bounds.left >= 0.0);
                    assert!(components.content_bounds.top >= 0.0);
                    assert!(components.content_bounds.right <= bounds.right);
                    assert!(components.content_bounds.bottom <= bounds.bottom);

                    // Time bounds strictly contained in content bounds
                    assert!(components.time_bounds.left >= components.content_bounds.left);
                    assert!(components.time_bounds.right <= components.content_bounds.right);
                    assert!(components.time_bounds.top >= components.content_bounds.top);
                    assert!(components.time_bounds.bottom <= components.content_bounds.bottom);

                    // Date bounds strictly contained in content bounds
                    assert!(components.date_bounds.left >= components.content_bounds.left);
                    assert!(components.date_bounds.right <= components.content_bounds.right);
                    assert!(components.date_bounds.top >= components.content_bounds.top);
                    assert!(components.date_bounds.bottom <= components.content_bounds.bottom);

                    // Corners of time and date within notch geometry
                    assert!(
                        dims.contains_point(
                            components.time_bounds.left,
                            components.time_bounds.top
                        )
                    );
                    assert!(
                        dims.contains_point(
                            components.time_bounds.right,
                            components.time_bounds.top
                        )
                    );
                    assert!(dims.contains_point(
                        components.date_bounds.left,
                        components.date_bounds.bottom
                    ));
                    assert!(dims.contains_point(
                        components.date_bounds.right,
                        components.date_bounds.bottom
                    ));
                }
                _ => panic!("Expected ResolvedLayout::Expanded"),
            }
        }
    }

    #[test]
    fn test_expanded_clock_date_centered_composition() {
        for dpi in [96, 120, 137, 144, 168, 192, 288] {
            let dims = NotchDimensions::from_state_and_dpi(NotchState::Expanded, dpi);
            let layout = resolve_layout(&dims);
            match layout {
                ResolvedLayout::Expanded { bounds, components } => {
                    let notch_cx = bounds.width() / 2.0;
                    let time_cx =
                        (components.time_bounds.left + components.time_bounds.right) / 2.0;
                    let date_cx =
                        (components.date_bounds.left + components.date_bounds.right) / 2.0;
                    let content_cx =
                        (components.content_bounds.left + components.content_bounds.right) / 2.0;

                    // Perfect horizontal optical alignment with notch center
                    assert!(
                        (time_cx - notch_cx).abs() <= 1.0,
                        "Time must center on notch at {dpi} DPI"
                    );
                    assert!(
                        (date_cx - notch_cx).abs() <= 1.0,
                        "Date must center on notch at {dpi} DPI"
                    );
                    assert!(
                        (content_cx - notch_cx).abs() <= 1.0,
                        "Content must center on notch at {dpi} DPI"
                    );

                    // Vertical optical stack balance: gap between content_top and time_top must be >= 0
                    assert!(components.time_bounds.top >= components.content_bounds.top);
                    assert!(components.content_bounds.bottom >= components.date_bounds.bottom);
                }
                _ => panic!("Expected ResolvedLayout::Expanded"),
            }
        }
    }

    const ALL_DPIS: [u32; 7] = [96, 120, 137, 144, 168, 192, 288];

    fn inside(inner: &RectF, outer: &RectF) -> bool {
        inner.left >= outer.left - 0.001
            && inner.right <= outer.right + 0.001
            && inner.top >= outer.top - 0.001
            && inner.bottom <= outer.bottom + 0.001
    }

    fn overlaps(a: &RectF, b: &RectF) -> bool {
        a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
    }

    fn corners(r: &RectF) -> [(f32, f32); 4] {
        [
            (r.left, r.top),
            (r.right, r.top),
            (r.left, r.bottom),
            (r.right, r.bottom),
        ]
    }

    fn media_and_content(dpi: u32, art: bool) -> (NotchDimensions, MediaLayout, RectF) {
        let dims = NotchDimensions::from_state_and_dpi(NotchState::Expanded, dpi);
        let media = resolve_media_layout(
            &dims,
            MediaShape {
                artwork: art,
                secondary_lines: 2,
            },
        )
        .expect("expanded media layout");
        let ResolvedLayout::Expanded { components, .. } = resolve_layout(&dims) else {
            panic!("Expected expanded layout");
        };
        (dims, media, components.content_bounds)
    }

    #[test]
    fn test_media_layout_absent_when_collapsed() {
        let dims = NotchDimensions::from_state_and_dpi(NotchState::Collapsed, 96);
        assert!(resolve_media_layout(&dims, MediaShape::FULL).is_none());
        assert!(
            resolve_media_layout(
                &dims,
                MediaShape {
                    artwork: false,
                    secondary_lines: 2
                }
            )
            .is_none()
        );
    }

    #[test]
    fn test_media_layout_reference_values_at_96_dpi() {
        let (_, m, c) = media_and_content(96, true);
        assert_eq!(c, RectF::new(42.0, 26.0, 558.0, 98.0));
        assert_eq!(m.artwork_bounds, Some(RectF::new(33.0, 29.0, 105.0, 101.0)));
        // Title row ends at the visualizer slot (14 px of bars + 8 px gap)
        assert_eq!(m.title_bounds, RectF::new(119.0, 26.0, 408.0, 46.0));
        assert_eq!(m.visualizer_bounds, RectF::new(416.0, 30.0, 430.0, 42.0));
        // Scrubber strip: right after Next's hit area, to the text column end
        assert_eq!(m.timeline_bounds, RectF::new(218.0, 76.0, 430.0, 98.0));
        assert_eq!(m.artist_bounds, RectF::new(119.0, 46.0, 430.0, 62.0));
        assert_eq!(m.source_bounds, RectF::new(119.0, 62.0, 430.0, 76.0));
        assert_eq!(m.time_bounds, RectF::new(446.0, 26.0, 558.0, 46.0));
        assert_eq!(m.date_bounds, RectF::new(446.0, 46.0, 558.0, 62.0));
        // Controls start at the text edge: Previous glyph (2 x 7 px) left edge = 119
        assert_eq!(m.previous_bounds, RectF::new(110.0, 76.0, 142.0, 104.0));
        assert_eq!(m.play_pause_bounds, RectF::new(148.0, 76.0, 180.0, 104.0));
        assert_eq!(m.next_bounds, RectF::new(186.0, 76.0, 218.0, 104.0));
        assert_eq!(
            m.control_visual_bounds(MediaControl::PlayPause),
            RectF::new(153.0, 76.0, 175.0, 98.0)
        );
    }

    #[test]
    fn test_media_layout_without_artwork_uses_full_text_column() {
        let (_, m, c) = media_and_content(96, false);
        assert_eq!(m.artwork_bounds, None);
        assert_eq!(m.title_bounds.left, c.left);
        let (_, with_art, _) = media_and_content(96, true);
        assert_eq!(m.title_bounds.right, with_art.title_bounds.right);
        assert_eq!(
            m.time_bounds, with_art.time_bounds,
            "clock column is stable"
        );
    }

    #[test]
    fn test_media_layout_containment_and_no_overlap_all_dpis() {
        for dpi in ALL_DPIS {
            for art in [true, false] {
                let (dims, m, c) = media_and_content(dpi, art);
                // Everything drawn stays inside the content bounds
                let mut visual = vec![
                    m.title_bounds,
                    m.artist_bounds,
                    m.source_bounds,
                    m.time_bounds,
                    m.date_bounds,
                ];
                visual.extend(m.artwork_bounds);
                visual.extend(MediaControl::ALL.map(|k| m.control_visual_bounds(k)));
                // Content area, grown left to the artwork at the notch edge
                let grown = RectF::new(
                    m.artwork_bounds.map_or(c.left, |a| a.left),
                    c.top,
                    c.right,
                    m.artwork_bounds
                        .map_or(c.bottom, |a| a.bottom.max(c.bottom)),
                );
                for (i, r) in visual.iter().enumerate() {
                    assert!(
                        inside(r, &grown),
                        "visual {i} escapes content at {dpi} DPI: {r:?}"
                    );
                    assert!(
                        r.width() > 0.0 && r.height() > 0.0,
                        "visual {i} empty at {dpi}"
                    );
                    for (j, other) in visual.iter().enumerate().skip(i + 1) {
                        assert!(!overlaps(r, other), "visuals {i}/{j} overlap at {dpi} DPI");
                    }
                }
                // Hit areas stay inside the notch contour and never cover text/artwork
                for k in MediaControl::ALL {
                    let hit = m.control_bounds(k);
                    for (x, y) in corners(&hit) {
                        assert!(
                            dims.contains_point(x, y),
                            "{k:?} hit outside notch at {dpi}"
                        );
                    }
                    for r in visual.iter().take(5).chain(m.artwork_bounds.iter()) {
                        assert!(!overlaps(&hit, r), "{k:?} hit covers content at {dpi}");
                    }
                }
                assert!(m.artist_bounds.top >= m.title_bounds.bottom);
                assert!(m.source_bounds.top >= m.artist_bounds.bottom);
                assert!(m.play_pause_bounds.top >= m.source_bounds.bottom);
            }
        }
    }

    #[test]
    fn test_media_artwork_square_and_inside_notch_all_dpis() {
        for dpi in ALL_DPIS {
            let (dims, m, c) = media_and_content(dpi, true);
            let a = m.artwork_bounds.unwrap();
            assert!(
                (a.width() - a.height()).abs() < 0.001,
                "artwork not square at {dpi}"
            );
            assert!(
                (a.height() - c.height()).abs() <= 1.0,
                "artwork spans content height"
            );
            // The drawn (rounded, concentric) cover nests inside the notch's corner
            let p = crate::config::CornerProfile::new(
                dims.curvature.bottom_radius - (dims.notch_height() - a.bottom),
                1.0,
                a.width() / 2.0,
            );
            for i in 0..=10 {
                let (u, v) = p.point(i as f32 / 10.0);
                let (x, y) = (a.left + u, a.bottom - p.span + v);
                assert!(
                    dims.contains_point(x, y),
                    "artwork corner outside notch at {dpi}"
                );
            }
            // Same inset from the side wall as from the bottom edge
            assert!(
                ((a.left - dims.shadow_margin_x - dims.curvature.top_transition_radius)
                    - (dims.notch_height() - a.bottom))
                    .abs()
                    <= 0.5,
                "artwork not nested in the corner at {dpi}"
            );
            for (x, y) in [(a.left, a.top), (a.right, a.top), (a.right, a.bottom)] {
                assert!(
                    dims.contains_point(x, y),
                    "artwork corner outside notch at {dpi}"
                );
            }
            assert!(
                m.title_bounds.left > a.right,
                "text starts right of artwork"
            );
        }
    }

    #[test]
    fn test_media_controls_aligned_to_text_and_evenly_spaced() {
        for dpi in ALL_DPIS {
            for art in [true, false] {
                let (dims, m, c) = media_and_content(dpi, art);
                let icon = (BASE_MEDIA_ICON_SIZE * dims.scale).round();
                let prev_cx = (m.previous_bounds.left + m.previous_bounds.right) / 2.0;
                let glyph_left = prev_cx - (icon * SKIP_GLYPH_HALF_WIDTH).round();
                if art {
                    // Previous glyph shares the text column's left edge
                    assert!(
                        (glyph_left - m.title_bounds.left).abs() <= 0.5,
                        "align at {dpi}"
                    );
                } else {
                    // At the content edge the backdrop stays inside the content
                    let v = m.control_visual_bounds(MediaControl::Previous);
                    assert!(v.left >= c.left - 0.001, "backdrop escapes at {dpi}");
                    assert!(
                        glyph_left >= m.title_bounds.left,
                        "glyph before text at {dpi}"
                    );
                }
                let gap_l = m.play_pause_bounds.left - m.previous_bounds.right;
                let gap_r = m.next_bounds.left - m.play_pause_bounds.right;
                assert!((gap_l - gap_r).abs() < 0.001 && gap_l > 0.0);
                assert_eq!(m.previous_bounds.width(), m.next_bounds.width());
                assert_eq!(m.previous_bounds.top, m.next_bounds.top);
                // The group is compact: it never reaches the clock column
                assert!(m.next_bounds.right < m.time_bounds.left);
            }
        }
    }

    #[test]
    fn test_media_layout_dpi_scaling() {
        let (_, base, _) = media_and_content(96, true);
        for dpi in ALL_DPIS {
            let (_, m, _) = media_and_content(dpi, true);
            let s = dpi as f32 / 96.0;
            let w = m.play_pause_bounds.width();
            let h = m.play_pause_bounds.height();
            assert!((w - (base.play_pause_bounds.width() * s).round()).abs() <= 0.001);
            // (the hit row gives up what the visual row loses to rounding: <= 1.5 px)
            assert!((h - (base.play_pause_bounds.height() * s).round()).abs() <= 1.5);
            // Hit target stays about 28x28 DIP at every scale
            assert!(
                w / s >= 28.0 - 0.5 && h / s >= 28.0 - 1.5,
                "hit target too small at {dpi}"
            );
            assert!((m.title_bounds.height() - (20.0 * s).round()).abs() <= 0.001);
            let a = m.artwork_bounds.unwrap();
            assert!(
                (a.width() - (72.0 * s).round()).abs() <= 1.0,
                "artwork scales at {dpi}"
            );
        }
    }

    #[test]
    fn test_media_hit_testing() {
        for dpi in ALL_DPIS {
            let (dims, m, _) = media_and_content(dpi, true);
            let center = |r: RectF| ((r.left + r.right) / 2.0, (r.top + r.bottom) / 2.0);
            for control in MediaControl::ALL {
                let (x, y) = center(m.control_bounds(control));
                assert_eq!(m.control_at(x, y), Some(control), "{control:?} at {dpi}");
                // The visual backdrop center is also a hit
                let (vx, vy) = center(m.control_visual_bounds(control));
                assert_eq!(m.control_at(vx, vy), Some(control));
            }
            // Gaps, text, artwork and clock are not controls
            let gap_x = (m.previous_bounds.right + m.play_pause_bounds.left) / 2.0;
            let (_, ctrl_y) = center(m.play_pause_bounds);
            assert_eq!(m.control_at(gap_x, ctrl_y), None);
            for r in [
                m.title_bounds,
                m.source_bounds,
                m.time_bounds,
                m.artwork_bounds.unwrap(),
            ] {
                let (x, y) = center(r);
                assert_eq!(m.control_at(x, y), None);
            }
            // Outside the notch body stays transparent
            assert!(!dims.contains_point(1.0, 1.0));
            assert!(!dims.contains_point(dims.width as f32 / 2.0, dims.height as f32 - 1.0));
            assert_eq!(m.control_at(1.0, 1.0), None);
        }
    }

    #[test]
    fn test_media_layout_contained_during_animation() {
        use crate::config::AnimationState;
        for dpi in [96, 144, 192] {
            for target in [NotchState::Expanded, NotchState::Collapsed] {
                let start = match target {
                    NotchState::Expanded => NotchState::Collapsed,
                    NotchState::Collapsed => NotchState::Expanded,
                };
                let mut anim = AnimationState::new(start, target);
                for step in 0..=20 {
                    anim.set_progress(step as f32 / 20.0);
                    let dims = anim.current_dimensions(dpi);
                    if let Some(m) = resolve_media_layout(&dims, MediaShape::FULL) {
                        // The stack starts inside the content area; overflow is clipped
                        let ResolvedLayout::Expanded { components, .. } = resolve_layout(&dims)
                        else {
                            unreachable!()
                        };
                        let c = components.content_bounds;
                        assert!(m.title_bounds.top >= c.top);
                        let a = m.artwork_bounds.unwrap();
                        let wall = dims.shadow_margin_x + dims.curvature.top_transition_radius;
                        assert!(a.left >= wall && a.top >= c.top - 0.001);
                        assert!(m.time_bounds.right <= c.right + 0.001);
                    }
                }
            }
        }
    }

    #[test]
    fn test_media_block_rebalances_for_missing_lines_all_dpis() {
        for dpi in ALL_DPIS {
            for artwork in [true, false] {
                let dims = NotchDimensions::from_state_and_dpi(NotchState::Expanded, dpi);
                let full = resolve_media_layout(
                    &dims,
                    MediaShape {
                        artwork,
                        secondary_lines: 2,
                    },
                )
                .unwrap();
                let ResolvedLayout::Expanded { components, .. } = resolve_layout(&dims) else {
                    unreachable!()
                };
                let c = components.content_bounds;
                for lines in [0u32, 1] {
                    let m = resolve_media_layout(
                        &dims,
                        MediaShape {
                            artwork,
                            secondary_lines: lines,
                        },
                    )
                    .unwrap();
                    // Controls follow the last shown line (no empty row), with at
                    // least the full layout's breathing room
                    let last_line_bottom = match lines {
                        0 => m.title_bounds.bottom,
                        _ => m.artist_bounds.bottom,
                    };
                    let gap = m.play_pause_bounds.top - last_line_bottom;
                    let full_gap = full.play_pause_bounds.top - full.source_bounds.bottom;
                    assert!(
                        gap >= full_gap - 0.001
                            && gap <= (BASE_MEDIA_CONTROLS_SPACING * dims.scale).round() + 0.001,
                        "empty row left at {dpi}, {lines} lines"
                    );
                    // The shorter block is re-centered: equal slack above and below (+/-1 px)
                    let block_bottom = m.control_visual_bounds(MediaControl::PlayPause).bottom;
                    let above = m.title_bounds.top - c.top;
                    let below = c.bottom - block_bottom;
                    assert!(
                        (above - below).abs() <= 1.5,
                        "not centered at {dpi}: {above} vs {below}"
                    );
                    // Clock column and artwork never move with the metadata
                    assert_eq!(m.time_bounds, full.time_bounds);
                    assert_eq!(m.date_bounds, full.date_bounds);
                    assert_eq!(m.artwork_bounds, full.artwork_bounds);
                    // Everything drawn stays inside the content bounds
                    for k in MediaControl::ALL {
                        assert!(inside(&m.control_visual_bounds(k), &c));
                    }
                }
            }
        }
    }

    #[test]
    fn test_visualizer_and_timeline_slots_all_dpis() {
        for dpi in ALL_DPIS {
            // Collapsed: inside the existing notch body, right end of the content
            let dims = NotchDimensions::from_state_and_dpi(NotchState::Collapsed, dpi);
            let l = resolve_collapsed_layout(&dims);
            let v = l.visualizer_bounds;
            assert!(
                inside(&v, &l.content_bounds),
                "collapsed viz in content at {dpi}"
            );
            assert_eq!(v.right, l.content_bounds.right);
            for (x, y) in corners(&v) {
                assert!(
                    dims.contains_point(x, y),
                    "collapsed viz inside notch at {dpi}"
                );
            }
            assert!(v.width() > 0.0 && v.height() > 0.0);

            for art in [true, false] {
                let (_, m, c) = media_and_content(dpi, art);
                let v = m.visualizer_bounds;
                assert!(inside(&v, &c), "expanded viz in content at {dpi}");
                // End of the title row, clear of the title and the clock column
                assert!(v.top >= m.title_bounds.top && v.bottom <= m.title_bounds.bottom);
                assert!(
                    m.title_bounds.right < v.left,
                    "title stops before viz at {dpi}"
                );
                assert_eq!(
                    v.right, m.artist_bounds.right,
                    "aligned to the text column end"
                );
                for r in [m.time_bounds, m.date_bounds, m.artist_bounds] {
                    assert!(!overlaps(&v, &r), "viz overlaps at {dpi}");
                }
                // Scrubber strip: in the controls row, right of every control
                let t = m.timeline_bounds;
                assert!(t.right <= c.right && t.bottom <= c.bottom + 0.001);
                assert_eq!(t.right, m.artist_bounds.right);
                for k in MediaControl::ALL {
                    assert!(
                        !overlaps(&t, &m.control_bounds(k)),
                        "{k:?} hit vs strip at {dpi}"
                    );
                }
                let pp = m.control_visual_bounds(MediaControl::PlayPause);
                assert!(
                    ((t.top + t.bottom) / 2.0 - (pp.top + pp.bottom) / 2.0).abs() < 0.001,
                    "strip centered on the controls at {dpi}"
                );
                assert!(
                    t.width() > 100.0 * dims.scale,
                    "room for the scrubber at {dpi}"
                );
            }
        }
    }
}
