#![allow(dead_code)]

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

use crate::config::{NotchDimensions, NotchState};

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
        assert_eq!(dims_e.height, 120);
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
}
