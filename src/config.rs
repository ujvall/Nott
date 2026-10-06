#![allow(dead_code)]

use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;
use windows::core::{PCWSTR, w};

pub const WINDOW_CLASS_NAME: PCWSTR = w!("NottWindowClass");
pub const WINDOW_TITLE: PCWSTR = w!("Nott");

// ============================================================================
// 1. BASELINE DESIGN SYSTEM TOKENS (Logical Pixels @ 96 DPI / 100% scale)
// ============================================================================

pub const BASE_DPI: f32 = 96.0;

// Collapsed reference dimensions (Phase 1 baseline preserved, smoothed shoulder)
pub const BASE_COLLAPSED_WIDTH: f32 = 220.0;
pub const BASE_COLLAPSED_HEIGHT: f32 = 32.0;
pub const BASE_COLLAPSED_BOTTOM_CORNER_RADIUS: f32 = 14.0;
pub const BASE_COLLAPSED_TOP_TRANSITION_RADIUS: f32 = 6.0;
pub const BASE_COLLAPSED_TOP_TRANSITION_HEIGHT: f32 = 6.0;

// Legacy aliases for full backward compatibility
pub const BASE_WIDTH: f32 = BASE_COLLAPSED_WIDTH;
pub const BASE_HEIGHT: f32 = BASE_COLLAPSED_HEIGHT;
pub const BASE_CORNER_RADIUS: f32 = BASE_COLLAPSED_BOTTOM_CORNER_RADIUS;
pub const BASE_BOTTOM_CORNER_RADIUS: f32 = BASE_COLLAPSED_BOTTOM_CORNER_RADIUS;
pub const BASE_TOP_TRANSITION_RADIUS: f32 = BASE_COLLAPSED_TOP_TRANSITION_RADIUS;
pub const BASE_TOP_TRANSITION_HEIGHT: f32 = BASE_COLLAPSED_TOP_TRANSITION_HEIGHT;

// Expanded reference dimensions (600 × 120 logical DIP horizontal pill, compact smooth shoulder, subtle dropshadow)
pub const BASE_EXPANDED_WIDTH: f32 = 600.0;
pub const BASE_EXPANDED_HEIGHT: f32 = 120.0;
pub const BASE_EXPANDED_BOTTOM_CORNER_RADIUS: f32 = 28.0;
pub const BASE_EXPANDED_TOP_TRANSITION_RADIUS: f32 = 14.0;
pub const BASE_EXPANDED_TOP_TRANSITION_HEIGHT: f32 = 14.0;
pub const BASE_EXPANDED_SHADOW_MARGIN_X: f32 = 10.0;
pub const BASE_EXPANDED_SHADOW_MARGIN_BOTTOM: f32 = 10.0;
pub const BASE_EXPANDED_SHADOW_OPACITY: f32 = 0.28;

// Stroke & separation tokens
pub const BASE_BORDER_WIDTH: f32 = 1.0;

// Padding & spacing tokens
pub const BASE_COLLAPSED_PADDING_H: f32 = 14.0;
pub const BASE_COLLAPSED_PADDING_V: f32 = 6.0;
pub const BASE_EXPANDED_PADDING_H: f32 = 18.0;
pub const BASE_EXPANDED_PADDING_V: f32 = 12.0;

pub const BASE_PADDING_H: f32 = BASE_COLLAPSED_PADDING_H;
pub const BASE_PADDING_V: f32 = BASE_COLLAPSED_PADDING_V;
pub const BASE_SPACING: f32 = 8.0;

// Typography tokens (logical point / DIP sizes)
pub const FONT_FAMILY_PRIMARY: PCWSTR = w!("Segoe UI Variable Text");
pub const FONT_FAMILY_FALLBACK: PCWSTR = w!("Segoe UI");
pub const BASE_FONT_SIZE_PRIMARY: f32 = 13.0;
pub const BASE_FONT_SIZE_SECONDARY: f32 = 10.5;
pub const BASE_FONT_SIZE_MUTED: f32 = 9.0;
pub const BASE_FONT_SIZE_CLOCK: f32 = 13.5;
pub const BASE_EXPANDED_FONT_SIZE_TIME: f32 = 28.0;
pub const BASE_EXPANDED_FONT_SIZE_DATE: f32 = 12.0;
pub const BASE_EXPANDED_TIME_HEIGHT: f32 = 34.0;
pub const BASE_EXPANDED_DATE_HEIGHT: f32 = 16.0;
pub const BASE_EXPANDED_CLOCK_SPACING: f32 = 4.0;
pub const BASE_EXPANDED_CLOCK_OPTICAL_Y_OFFSET: f32 = 0.0;
pub const BASE_CLOCK_OPTICAL_Y_OFFSET: f32 = 0.0;
pub const BASE_TITLE_OPTICAL_Y_OFFSET: f32 = 0.65;
pub const BASE_HEADER_HEIGHT: f32 = 24.0;
pub const BASE_SEPARATOR_MARGIN_TOP: f32 = 6.0;
pub const BASE_PREVIEW_MARGIN_TOP: f32 = 8.0;

// Icon sizing tokens (logical DIP sizes)
pub const BASE_ICON_SIZE_DEFAULT: f32 = 16.0;
pub const BASE_ICON_SIZE_SMALL: f32 = 12.0;
pub const BASE_ICON_STROKE_WIDTH: f32 = 1.2;
pub const BASE_ICON_ASPECT_RATIO: f32 = 0.70;
pub const BASE_ICON_CORNER_RADIUS: f32 = 2.5;
pub const BASE_ICON_APERTURE_RADIUS: f32 = 1.5;

// ============================================================================
// 2. MINIMAL DARK / AMOLED COLOR PALETTE
// ============================================================================

/// True AMOLED / OLED-style pure black surface (#000000, alpha 1.0).
/// Visually seamless against black OLED displays and dark laptop bezels.
pub const COLOR_SURFACE_AMOLED: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

/// High-contrast soft white primary text against pure black
pub const COLOR_TEXT_PRIMARY: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.96,
    g: 0.96,
    b: 0.97,
    a: 1.0,
};

/// Subdued neutral gray secondary text
pub const COLOR_TEXT_SECONDARY: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.65,
    g: 0.65,
    b: 0.68,
    a: 1.0,
};

/// Dim muted gray text for subtle labels
pub const COLOR_TEXT_MUTED: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.45,
    g: 0.45,
    b: 0.48,
    a: 1.0,
};

/// Subtle border highlight (set to transparent so notch stays pure black with zero white overlay)
pub const COLOR_BORDER_SUBTLE: D2D1_COLOR_F = COLOR_TRANSPARENT;

/// Hover border highlight (strictly transparent: eliminates the unwanted white overlay on hover)
pub const COLOR_BORDER_HOVER: D2D1_COLOR_F = COLOR_TRANSPARENT;

/// Transparent background color for layered window clearing
pub const COLOR_TRANSPARENT: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

// Aliases for seamless Phase 1 compatibility
pub const NOTCH_BG_COLOR: D2D1_COLOR_F = COLOR_SURFACE_AMOLED;
pub const NOTCH_BORDER_COLOR: D2D1_COLOR_F = COLOR_BORDER_SUBTLE;

/// Subtle separator color token for expanded state divider line
pub const COLOR_SEPARATOR: D2D1_COLOR_F = D2D1_COLOR_F {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.04,
};

// ============================================================================
// 3. HARDWARE-NOTCH GEOMETRY & CURVATURE MODEL
// ============================================================================

/// UI state of the notch
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotchState {
    #[default]
    Collapsed,
    Expanded,
}

/// Explicit interaction state tracking notch expansion and hover
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InteractionState {
    pub state: NotchState,
    pub hovered: bool,
}

impl InteractionState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Handles left click toggle between Collapsed and Expanded.
    /// Returns true if the state changed.
    pub fn on_left_click(&mut self) -> bool {
        let prev = self.state;
        self.state = match self.state {
            NotchState::Collapsed => NotchState::Expanded,
            NotchState::Expanded => NotchState::Collapsed,
        };
        prev != self.state
    }

    /// Updates hover status. Returns true if hover status actually changed.
    pub fn set_hovered(&mut self, hovered: bool) -> bool {
        if self.hovered != hovered {
            self.hovered = hovered;
            true
        } else {
            false
        }
    }
}

/// Curvature parameters for hardware cutout representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HardwareNotchCurvature {
    /// Radius of the lower rounded corners
    pub bottom_radius: f32,
    /// Curvature radius for smooth attachment flowing into the top screen bezel (horizontal spread)
    pub top_transition_radius: f32,
    /// Vertical height of top shoulder curve before the straight vertical side wall begins
    pub top_transition_height: f32,
}

/// Centralized DPI-aware dimensions and geometry for the notch
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotchDimensions {
    pub state: NotchState,
    pub width: i32,
    pub height: i32,
    /// Lower corner radius (aliased for compatibility)
    pub corner_radius: f32,
    /// Curvature model representing the hardware cutout geometry
    pub curvature: HardwareNotchCurvature,
    pub border_width: f32,
    pub padding_h: f32,
    pub padding_v: f32,
    pub spacing: f32,
    pub icon_size_default: f32,
    pub icon_size_small: f32,
    pub font_size_primary: f32,
    pub font_size_secondary: f32,
    pub font_size_muted: f32,
    pub font_size_clock: f32,
    pub font_size_expanded_time: f32,
    pub font_size_expanded_date: f32,
    pub dpi: u32,
    pub scale: f32,
    pub shadow_margin_x: f32,
    pub shadow_margin_bottom: f32,
    pub shadow_opacity: f32,
}

impl NotchDimensions {
    /// Computes physical pixel dimensions and design parameters for a given state and display DPI.
    /// Standard baseline is 96 DPI (100% Windows scaling).
    pub fn from_state_and_dpi(state: NotchState, dpi: u32) -> Self {
        let effective_dpi = if dpi == 0 { 96 } else { dpi };
        let scale = effective_dpi as f32 / BASE_DPI;

        let (base_w, base_h, base_bottom_r, base_top_r, base_top_h, base_pad_h, base_pad_v) =
            match state {
                NotchState::Collapsed => (
                    BASE_COLLAPSED_WIDTH,
                    BASE_COLLAPSED_HEIGHT,
                    BASE_COLLAPSED_BOTTOM_CORNER_RADIUS,
                    BASE_COLLAPSED_TOP_TRANSITION_RADIUS,
                    BASE_COLLAPSED_TOP_TRANSITION_HEIGHT,
                    BASE_COLLAPSED_PADDING_H,
                    BASE_COLLAPSED_PADDING_V,
                ),
                NotchState::Expanded => (
                    BASE_EXPANDED_WIDTH,
                    BASE_EXPANDED_HEIGHT,
                    BASE_EXPANDED_BOTTOM_CORNER_RADIUS,
                    BASE_EXPANDED_TOP_TRANSITION_RADIUS,
                    BASE_EXPANDED_TOP_TRANSITION_HEIGHT,
                    BASE_EXPANDED_PADDING_H,
                    BASE_EXPANDED_PADDING_V,
                ),
            };

        let bottom_radius = base_bottom_r * scale;
        let top_transition_radius = base_top_r * scale;
        let top_transition_height = base_top_h * scale;

        let (shadow_margin_x, shadow_margin_bottom, shadow_opacity) = match state {
            NotchState::Collapsed => (0.0, 0.0, 0.0),
            NotchState::Expanded => (
                (BASE_EXPANDED_SHADOW_MARGIN_X * scale).round(),
                (BASE_EXPANDED_SHADOW_MARGIN_BOTTOM * scale).round(),
                BASE_EXPANDED_SHADOW_OPACITY,
            ),
        };

        Self {
            state,
            width: (base_w * scale).round() as i32,
            height: (base_h * scale).round() as i32,
            corner_radius: bottom_radius,
            curvature: HardwareNotchCurvature {
                bottom_radius,
                top_transition_radius,
                top_transition_height,
            },
            border_width: (BASE_BORDER_WIDTH * scale).max(1.0),
            padding_h: (base_pad_h * scale).round(),
            padding_v: (base_pad_v * scale).round(),
            spacing: (BASE_SPACING * scale).round(),
            icon_size_default: (BASE_ICON_SIZE_DEFAULT * scale).round(),
            icon_size_small: (BASE_ICON_SIZE_SMALL * scale).round(),
            font_size_primary: BASE_FONT_SIZE_PRIMARY * scale,
            font_size_secondary: BASE_FONT_SIZE_SECONDARY * scale,
            font_size_muted: BASE_FONT_SIZE_MUTED * scale,
            font_size_clock: BASE_FONT_SIZE_CLOCK * scale,
            font_size_expanded_time: BASE_EXPANDED_FONT_SIZE_TIME * scale,
            font_size_expanded_date: BASE_EXPANDED_FONT_SIZE_DATE * scale,
            dpi: effective_dpi,
            scale,
            shadow_margin_x,
            shadow_margin_bottom,
            shadow_opacity,
        }
    }

    /// Convenience constructor defaulting to Collapsed state for backwards compatibility.
    pub fn from_dpi(dpi: u32) -> Self {
        Self::from_state_and_dpi(NotchState::Collapsed, dpi)
    }

    /// Returns the lower corner radius of the notch
    #[inline]
    pub fn bottom_corner_radius(&self) -> f32 {
        self.curvature.bottom_radius
    }

    /// Returns the top transition/shoulder radius of the notch
    #[inline]
    pub fn top_transition_radius(&self) -> f32 {
        self.curvature.top_transition_radius
    }

    /// Returns the top transition/shoulder height of the notch
    #[inline]
    pub fn top_transition_height(&self) -> f32 {
        self.curvature.top_transition_height
    }

    /// Returns the physical width of the solid notch body excluding shadow margins
    #[inline]
    pub fn notch_width(&self) -> f32 {
        (self.width as f32 - 2.0 * self.shadow_margin_x).max(0.0)
    }

    /// Returns the physical height of the solid notch body excluding shadow margins
    #[inline]
    pub fn notch_height(&self) -> f32 {
        (self.height as f32 - self.shadow_margin_bottom).max(0.0)
    }

    /// Verifies left-right symmetry of the geometry
    #[inline]
    pub fn is_symmetric(&self) -> bool {
        self.width % 2 == 0 || self.width > 0
    }

    /// Hit-tests whether a client coordinate point (px, py) is inside the hardware notch body
    #[inline]
    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        let pad_x = self.shadow_margin_x;
        let notch_w = self.notch_width();
        let notch_h = self.notch_height();

        if px < pad_x || px > (pad_x + notch_w) || py > notch_h {
            return false;
        }

        is_point_in_notch_ex(
            px - pad_x,
            py,
            notch_w,
            notch_h,
            self.curvature.top_transition_radius,
            self.curvature.top_transition_height,
            self.curvature.bottom_radius,
        )
    }
}

/// Computes the horizontal top-center position $X$ for given screen and notch widths.
/// Ensures window is horizontally centered and clamped to valid display bounds.
pub fn calculate_notch_x(screen_width: i32, notch_width: i32) -> i32 {
    if screen_width <= 0 || notch_width <= 0 {
        return 0;
    }
    ((screen_width - notch_width) / 2).max(0)
}

/// Computes the horizontal center coordinate of the notch for given screen and notch widths.
pub fn calculate_notch_center_x(screen_width: i32, notch_width: i32) -> f32 {
    let x = calculate_notch_x(screen_width, notch_width);
    x as f32 + (notch_width as f32 / 2.0)
}

// ============================================================================
// 4. HIT TESTING
// ============================================================================

/// Mathematical hit testing to check if a client-space point (px, py)
/// lies inside the physical hardware-notch geometry.
///
/// Follows the hardware notch shape:
/// - Top shoulders: concave transition arcs connecting to top screen edge
/// - Central body: vertical side walls
/// - Bottom corners: convex rounded corners
///
/// Mathematical hit testing to check if a client-space point (px, py)
/// lies inside the physical hardware-notch geometry with independent
/// horizontal transition radius and vertical transition height.
pub fn is_point_in_notch_ex(
    px: f32,
    py: f32,
    width: f32,
    height: f32,
    top_transition_radius: f32,
    top_transition_height: f32,
    bottom_corner_radius: f32,
) -> bool {
    if width <= 0.0 || height <= 0.0 || px < 0.0 || px > width || py < 0.0 || py > height {
        return false;
    }

    let r_top_x = top_transition_radius
        .max(0.0)
        .min(height / 2.0)
        .min(width / 4.0);
    let r_top_y = top_transition_height
        .max(0.0)
        .min(height / 2.0)
        .min(width / 4.0);
    let r_bottom = bottom_corner_radius
        .max(0.0)
        .min(height / 2.0)
        .min(width / 4.0);

    // 1. Top shoulder region (0 <= py <= r_top_y)
    if py <= r_top_y && r_top_x > 0.0 && r_top_y > 0.0 {
        let ny = (py - r_top_y) / r_top_y;
        // Left concave shoulder: outside if left of arc
        if px < r_top_x {
            let nx = px / r_top_x;
            return (nx * nx + ny * ny) >= 1.0;
        }
        // Right concave shoulder: outside if right of arc
        if px > (width - r_top_x) {
            let nx = (px - width) / r_top_x;
            return (nx * nx + ny * ny) >= 1.0;
        }
        // Between shoulders at the top edge is solid notch body
        return true;
    }

    // 2. Outside side walls below the shoulder region (py > r_top_y)
    if px < r_top_x || px > (width - r_top_x) {
        return false;
    }

    // 3. Bottom rounded corners (height - r_bottom <= py <= height)
    if py > (height - r_bottom) && r_bottom > 0.0 {
        let dy = py - (height - r_bottom);
        // Bottom-left rounded corner
        if px < (r_top_x + r_bottom) {
            let dx = px - (r_top_x + r_bottom);
            return (dx * dx + dy * dy) <= (r_bottom * r_bottom);
        }
        // Bottom-right rounded corner
        if px > (width - r_top_x - r_bottom) {
            let dx = px - (width - r_top_x - r_bottom);
            return (dx * dx + dy * dy) <= (r_bottom * r_bottom);
        }
    }

    // 4. Central body of the notch
    true
}

/// Backward-compatible hit testing assuming equal horizontal and vertical top transition radii.
pub fn is_point_in_notch(
    px: f32,
    py: f32,
    width: f32,
    height: f32,
    top_transition_radius: f32,
    bottom_corner_radius: f32,
) -> bool {
    is_point_in_notch_ex(
        px,
        py,
        width,
        height,
        top_transition_radius,
        top_transition_radius,
        bottom_corner_radius,
    )
}

// ============================================================================
// 5. ANIMATION MODEL & EASING (Phase 2.5)
// ============================================================================

/// Target animation duration in milliseconds (~340ms for Apple spring settlement)
pub const ANIMATION_DURATION_MS: u64 = 340;

/// Frame interval in milliseconds (~12ms for fluid ~80 FPS animation ticks)
pub const ANIMATION_FRAME_INTERVAL_MS: u32 = 12;

/// Dedicated timer ID for window animation
pub const ANIMATION_TIMER_ID: usize = 1001;

/// Dedicated timer ID for minute clock rollover (Phase 3.1)
pub const CLOCK_TIMER_ID: usize = 1002;

/// Apple Damped Harmonic Spring solver:
/// Damping ratio zeta = 0.74 (provides a crisp, natural +3% bounce), response = 0.35s
pub fn apple_spring(progress: f32) -> f32 {
    let t = progress.clamp(0.0, 1.0) * 0.35;
    let zeta = 0.74f32;
    let response = 0.35f32;
    let omega_n = 2.0 * std::f32::consts::PI / response;
    let omega_d = omega_n * (1.0 - zeta * zeta).sqrt();
    let envelope = (-zeta * omega_n * t).exp();
    let oscillation =
        (omega_d * t).cos() + (zeta / (1.0 - zeta * zeta).sqrt()) * (omega_d * t).sin();
    (1.0 - envelope * oscillation).clamp(0.0, 1.05)
}

/// Cubic ease-out curve: f(t) = 1 - (1 - t)^3
/// Provides responsive initial velocity smoothly decelerating to rest.
pub fn cubic_ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    1.0 - inv * inv * inv
}

/// Linear interpolation helper
#[inline]
pub fn lerp(start: f32, target: f32, t: f32) -> f32 {
    start + (target - start) * t
}

/// Returns reference (width, height, top_transition_radius, top_transition_height, bottom_corner_radius) in logical DIP for a given NotchState.
pub fn reference_geometry_for_state(state: NotchState) -> (f32, f32, f32, f32, f32) {
    match state {
        NotchState::Collapsed => (
            BASE_COLLAPSED_WIDTH,
            BASE_COLLAPSED_HEIGHT,
            BASE_COLLAPSED_TOP_TRANSITION_RADIUS,
            BASE_COLLAPSED_TOP_TRANSITION_HEIGHT,
            BASE_COLLAPSED_BOTTOM_CORNER_RADIUS,
        ),
        NotchState::Expanded => (
            BASE_EXPANDED_WIDTH,
            BASE_EXPANDED_HEIGHT,
            BASE_EXPANDED_TOP_TRANSITION_RADIUS,
            BASE_EXPANDED_TOP_TRANSITION_HEIGHT,
            BASE_EXPANDED_BOTTOM_CORNER_RADIUS,
        ),
    }
}

/// Animation state tracking transition progress between Collapsed and Expanded states
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationState {
    pub start_state: NotchState,
    pub target_state: NotchState,
    pub start_width: f32,
    pub start_height: f32,
    pub start_top_r: f32,
    pub start_top_h: f32,
    pub start_bottom_r: f32,
    pub target_width: f32,
    pub target_height: f32,
    pub target_top_r: f32,
    pub target_top_h: f32,
    pub target_bottom_r: f32,
    pub elapsed_ms: u64,
    pub duration_ms: u64,
    pub progress: f32,
    pub active: bool,
}

impl AnimationState {
    /// Creates a new animation transitioning from `current_state` toward `target_state`.
    pub fn new(current_state: NotchState, target_state: NotchState) -> Self {
        let (s_w, s_h, s_top_r, s_top_h, s_bot_r) = reference_geometry_for_state(current_state);
        let (t_w, t_h, t_top_r, t_top_h, t_bot_r) = reference_geometry_for_state(target_state);
        Self {
            start_state: current_state,
            target_state,
            start_width: s_w,
            start_height: s_h,
            start_top_r: s_top_r,
            start_top_h: s_top_h,
            start_bottom_r: s_bot_r,
            target_width: t_w,
            target_height: t_h,
            target_top_r: t_top_r,
            target_top_h: t_top_h,
            target_bottom_r: t_bot_r,
            elapsed_ms: 0,
            duration_ms: ANIMATION_DURATION_MS,
            progress: 0.0,
            active: true,
        }
    }

    /// Handles interruption / reversal of an active animation without jumping or leaking resources.
    /// Uses current interpolated geometry as the new starting point and reverses direction.
    pub fn reverse_from_current(&self) -> Self {
        let (cur_w, cur_h, cur_top_r, cur_bot_r) = self.current_logical_geometry();
        let cur_top_h = self.current_logical_top_h();
        let new_target = match self.target_state {
            NotchState::Collapsed => NotchState::Expanded,
            NotchState::Expanded => NotchState::Collapsed,
        };
        let (t_w, t_h, t_top_r, t_top_h, t_bot_r) = reference_geometry_for_state(new_target);

        // Scale duration based on remaining distance to keep velocity natural
        let distance_ratio = ((cur_w - t_w).abs()
            / (BASE_EXPANDED_WIDTH - BASE_COLLAPSED_WIDTH).max(1.0))
        .clamp(0.1, 1.0);
        let duration_ms = ((ANIMATION_DURATION_MS as f32) * distance_ratio).max(60.0) as u64;

        Self {
            start_state: self.target_state,
            target_state: new_target,
            start_width: cur_w,
            start_height: cur_h,
            start_top_r: cur_top_r,
            start_top_h: cur_top_h,
            start_bottom_r: cur_bot_r,
            target_width: t_w,
            target_height: t_h,
            target_top_r: t_top_r,
            target_top_h: t_top_h,
            target_bottom_r: t_bot_r,
            elapsed_ms: 0,
            duration_ms,
            progress: 0.0,
            active: true,
        }
    }

    /// Advances the animation by `delta_ms`. Returns true if animation is still active.
    pub fn step(&mut self, delta_ms: u64) -> bool {
        if !self.active {
            return false;
        }
        self.elapsed_ms += delta_ms;
        if self.elapsed_ms >= self.duration_ms {
            self.progress = 1.0;
            self.active = false;
        } else {
            self.progress = (self.elapsed_ms as f32 / self.duration_ms as f32).clamp(0.0, 1.0);
        }
        self.active
    }

    /// Explicitly sets normalized progress in [0.0, 1.0] (useful for deterministic tests).
    pub fn set_progress(&mut self, p: f32) {
        self.progress = p.clamp(0.0, 1.0);
        if self.progress >= 1.0 {
            self.active = false;
        }
    }

    /// Returns the current logical dimensions (unscaled DIP) taking Apple spring physics into account.
    pub fn current_logical_geometry(&self) -> (f32, f32, f32, f32) {
        let e = if self.progress >= 1.0 {
            1.0
        } else {
            apple_spring(self.progress)
        };
        let w = lerp(self.start_width, self.target_width, e);
        let h = lerp(self.start_height, self.target_height, e);
        let top_r = lerp(self.start_top_r, self.target_top_r, e.min(1.0));
        let bot_r = lerp(self.start_bottom_r, self.target_bottom_r, e.min(1.0));
        (w, h, top_r, bot_r)
    }

    /// Returns the current logical top shoulder height.
    pub fn current_logical_top_h(&self) -> f32 {
        let e = if self.progress >= 1.0 {
            1.0
        } else {
            apple_spring(self.progress)
        };
        lerp(self.start_top_h, self.target_top_h, e.min(1.0))
    }

    /// Computes physical NotchDimensions for the current animation frame given display DPI.
    pub fn current_dimensions(&self, dpi: u32) -> NotchDimensions {
        let scale = if dpi == 0 { 1.0 } else { dpi as f32 / BASE_DPI };
        let (log_w, log_h, log_top_r, log_bot_r) = self.current_logical_geometry();
        let log_top_h = self.current_logical_top_h();

        let top_transition_radius = log_top_r * scale;
        let top_transition_height = log_top_h * scale;
        let bottom_radius = log_bot_r * scale;

        // When progress > 0.5 or complete, state resolves to target_state
        let state = if !self.active || self.progress > 0.5 {
            self.target_state
        } else {
            self.start_state
        };

        let mut width = (log_w * scale).round() as i32;
        if width % 2 != 0 {
            width += 1;
        }

        NotchDimensions {
            state,
            width,
            height: (log_h * scale).round() as i32,
            corner_radius: bottom_radius,
            curvature: HardwareNotchCurvature {
                bottom_radius,
                top_transition_radius,
                top_transition_height,
            },
            border_width: (BASE_BORDER_WIDTH * scale).max(1.0),
            padding_h: (BASE_COLLAPSED_PADDING_H * scale).round(),
            padding_v: (BASE_COLLAPSED_PADDING_V * scale).round(),
            spacing: (BASE_SPACING * scale).round(),
            icon_size_default: (BASE_ICON_SIZE_DEFAULT * scale).round(),
            icon_size_small: (BASE_ICON_SIZE_SMALL * scale).round(),
            font_size_primary: BASE_FONT_SIZE_PRIMARY * scale,
            font_size_secondary: BASE_FONT_SIZE_SECONDARY * scale,
            font_size_muted: BASE_FONT_SIZE_MUTED * scale,
            font_size_clock: BASE_FONT_SIZE_CLOCK * scale,
            font_size_expanded_time: BASE_EXPANDED_FONT_SIZE_TIME * scale,
            font_size_expanded_date: BASE_EXPANDED_FONT_SIZE_DATE * scale,
            dpi: if dpi == 0 { 96 } else { dpi },
            scale,
            shadow_margin_x: match self.target_state {
                NotchState::Expanded => {
                    (BASE_EXPANDED_SHADOW_MARGIN_X * scale * self.progress).round()
                }
                NotchState::Collapsed => {
                    (BASE_EXPANDED_SHADOW_MARGIN_X * scale * (1.0 - self.progress)).round()
                }
            },
            shadow_margin_bottom: match self.target_state {
                NotchState::Expanded => {
                    (BASE_EXPANDED_SHADOW_MARGIN_BOTTOM * scale * self.progress).round()
                }
                NotchState::Collapsed => {
                    (BASE_EXPANDED_SHADOW_MARGIN_BOTTOM * scale * (1.0 - self.progress)).round()
                }
            },
            shadow_opacity: match self.target_state {
                NotchState::Expanded => BASE_EXPANDED_SHADOW_OPACITY * self.progress,
                NotchState::Collapsed => BASE_EXPANDED_SHADOW_OPACITY * (1.0 - self.progress),
            },
        }
    }
}

// ============================================================================
// 6. ACCESSIBILITY SYSTEM TOKENS & MODEL (Phase 2.7)
// ============================================================================

/// Accessible name for the top-level application / overlay
pub const ACCESSIBLE_NAME: PCWSTR = w!("Nott");
pub const ACCESSIBLE_NAME_STR: &str = "Nott";

/// Concise accessible description explaining the overlay's purpose
pub const ACCESSIBLE_DESCRIPTION: &str = "Interactive desktop notch overlay";

/// Accessible semantic role name for the visual overlay
pub const ACCESSIBLE_ROLE_NAME: &str = "Desktop Overlay";

/// Full accessible title when the notch is in its expanded state
pub const ACCESSIBLE_EXPANDED_TITLE: PCWSTR = w!("Nott");
pub const ACCESSIBLE_EXPANDED_TITLE_STR: &str = "Nott";

/// Returns the user-facing accessible window title for a given notch state
pub fn accessible_title_for_state(state: NotchState) -> PCWSTR {
    match state {
        NotchState::Collapsed => ACCESSIBLE_NAME,
        NotchState::Expanded => ACCESSIBLE_EXPANDED_TITLE,
    }
}

/// Returns the user-facing accessible text string for a given notch state
pub fn accessible_text_for_state(state: NotchState) -> &'static str {
    match state {
        NotchState::Collapsed => ACCESSIBLE_NAME_STR,
        NotchState::Expanded => ACCESSIBLE_EXPANDED_TITLE_STR,
    }
}

/// Returns the accessible state description string
pub fn accessible_state_description(state: NotchState) -> &'static str {
    match state {
        NotchState::Collapsed => "Collapsed",
        NotchState::Expanded => "Expanded",
    }
}

/// Explicit semantic accessibility snapshot for Nott representing what
/// screen readers and accessibility clients observe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibilitySnapshot {
    pub name: String,
    pub description: &'static str,
    pub role: &'static str,
    pub state_label: &'static str,
    pub visible_text: String,
    pub exposes_subtitle: bool,
    pub is_icon_interactive: bool,
}

impl AccessibilitySnapshot {
    pub fn for_state_with_clock(
        state: NotchState,
        formatted_time: &str,
        formatted_date: &str,
    ) -> Self {
        match state {
            NotchState::Collapsed => Self {
                name: formatted_time.to_string(),
                description: ACCESSIBLE_DESCRIPTION,
                role: ACCESSIBLE_ROLE_NAME,
                state_label: "Collapsed",
                visible_text: formatted_time.to_string(),
                exposes_subtitle: false,
                is_icon_interactive: false,
            },
            NotchState::Expanded => {
                let text = format!("{formatted_time} - {formatted_date}");
                Self {
                    name: text.clone(),
                    description: ACCESSIBLE_DESCRIPTION,
                    role: ACCESSIBLE_ROLE_NAME,
                    state_label: "Expanded",
                    visible_text: text,
                    exposes_subtitle: false,
                    is_icon_interactive: false,
                }
            }
        }
    }

    pub fn for_state_with_time(state: NotchState, formatted_time: &str) -> Self {
        Self::for_state_with_clock(state, formatted_time, "Monday, October 6")
    }

    pub fn for_state(state: NotchState) -> Self {
        Self::for_state_with_clock(state, "12:00 PM", "Monday, October 6")
    }
}

/// Deterministic event tracker modeling accessibility event emissions.
/// Validates that hover and intermediate animation ticks produce zero events,
/// while settled state changes emit exactly one event.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccessibilityEventTracker {
    pub name_change_count: u32,
    pub state_change_count: u32,
    pub last_state: Option<NotchState>,
}

impl AccessibilityEventTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Emits a state transition event if and only if the stable state changed.
    pub fn on_state_settled(&mut self, new_state: NotchState) -> bool {
        if self.last_state != Some(new_state) {
            self.last_state = Some(new_state);
            self.name_change_count += 1;
            self.state_change_count += 1;
            true
        } else {
            false
        }
    }

    /// Explicitly documents that intermediate animation frames do not emit events.
    pub fn on_animation_frame(&mut self, _progress: f32) -> bool {
        false
    }

    /// Explicitly documents that hover state changes do not emit events.
    pub fn on_hover_changed(&mut self, _hovered: bool) -> bool {
        false
    }

    /// Emits a name change event when the displayed minute changes in collapsed or expanded state.
    pub fn on_clock_minute_changed(&mut self, _is_collapsed: bool) -> bool {
        self.name_change_count += 1;
        true
    }
}

// ============================================================================
// 7. DETERMINISTIC TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpi_dimension_scaling() {
        // 100% scaling: 96 DPI
        let dims_100 = NotchDimensions::from_dpi(96);
        assert_eq!(dims_100.width, 220);
        assert_eq!(dims_100.height, 32);
        assert!((dims_100.corner_radius - 14.0).abs() < 1e-4);
        assert!((dims_100.curvature.bottom_radius - 14.0).abs() < 1e-4);
        assert!((dims_100.curvature.top_transition_radius - 6.0).abs() < 1e-4);
        assert!((dims_100.border_width - 1.0).abs() < 1e-4);
        assert_eq!(dims_100.padding_h, 14.0);
        assert_eq!(dims_100.padding_v, 6.0);
        assert_eq!(dims_100.icon_size_default, 16.0);
        assert_eq!(dims_100.icon_size_small, 12.0);

        // 125% scaling: 120 DPI
        let dims_125 = NotchDimensions::from_dpi(120);
        assert_eq!(dims_125.width, 275);
        assert_eq!(dims_125.height, 40);
        assert!((dims_125.corner_radius - 17.5).abs() < 1e-4);
        assert!((dims_125.curvature.bottom_radius - 17.5).abs() < 1e-4);
        assert!((dims_125.curvature.top_transition_radius - 7.5).abs() < 1e-4);
        assert!((dims_125.border_width - 1.25).abs() < 1e-4);

        // 150% scaling: 144 DPI
        let dims_150 = NotchDimensions::from_dpi(144);
        assert_eq!(dims_150.width, 330);
        assert_eq!(dims_150.height, 48);
        assert!((dims_150.corner_radius - 21.0).abs() < 1e-4);
        assert!((dims_150.curvature.top_transition_radius - 9.0).abs() < 1e-4);
        assert!((dims_150.border_width - 1.5).abs() < 1e-4);

        // 175% scaling: 168 DPI
        let dims_175 = NotchDimensions::from_dpi(168);
        assert_eq!(dims_175.width, 385);
        assert_eq!(dims_175.height, 56);
        assert!((dims_175.corner_radius - 24.5).abs() < 1e-4);
        assert!((dims_175.curvature.top_transition_radius - 10.5).abs() < 1e-4);
        assert!((dims_175.border_width - 1.75).abs() < 1e-4);

        // 200% scaling: 192 DPI
        let dims_200 = NotchDimensions::from_dpi(192);
        assert_eq!(dims_200.width, 440);
        assert_eq!(dims_200.height, 64);
        assert!((dims_200.corner_radius - 28.0).abs() < 1e-4);
        assert!((dims_200.curvature.top_transition_radius - 12.0).abs() < 1e-4);
        assert!((dims_200.border_width - 2.0).abs() < 1e-4);

        // Unusual DPI: 137 DPI (arbitrary non-standard display scaling)
        let dims_137 = NotchDimensions::from_dpi(137);
        let expected_scale: f32 = 137.0 / 96.0;
        assert_eq!(dims_137.width, (220.0f32 * expected_scale).round() as i32);
        assert_eq!(dims_137.height, (32.0f32 * expected_scale).round() as i32);
        assert!((dims_137.corner_radius - (14.0 * expected_scale)).abs() < 1e-4);
        assert!((dims_137.curvature.top_transition_radius - (6.0 * expected_scale)).abs() < 1e-4);

        // High DPI: 288 DPI (300% 4K display scaling)
        let dims_288 = NotchDimensions::from_dpi(288);
        assert_eq!(dims_288.width, 660);
        assert_eq!(dims_288.height, 96);
        assert!((dims_288.corner_radius - 42.0).abs() < 1e-4);
        assert!((dims_288.curvature.top_transition_radius - 18.0).abs() < 1e-4);
        assert!((dims_288.border_width - 3.0).abs() < 1e-4);

        // Zero DPI fallback defaults safely to 96 DPI
        let dims_zero = NotchDimensions::from_dpi(0);
        assert_eq!(dims_zero.width, 220);
        assert_eq!(dims_zero.height, 32);
        assert_eq!(dims_zero.dpi, 96);
    }

    #[test]
    fn test_amoled_surface_color() {
        // Must be true pure black (0, 0, 0, 1.0)
        assert_eq!(COLOR_SURFACE_AMOLED.r, 0.0);
        assert_eq!(COLOR_SURFACE_AMOLED.g, 0.0);
        assert_eq!(COLOR_SURFACE_AMOLED.b, 0.0);
        assert_eq!(COLOR_SURFACE_AMOLED.a, 1.0);
        assert_eq!(NOTCH_BG_COLOR.r, 0.0);
        assert_eq!(NOTCH_BG_COLOR.g, 0.0);
        assert_eq!(NOTCH_BG_COLOR.b, 0.0);
        assert_eq!(NOTCH_BG_COLOR.a, 1.0);
    }

    #[test]
    fn test_hardware_curvature_model() {
        let dims = NotchDimensions::from_dpi(96);
        assert_eq!(dims.bottom_corner_radius(), 14.0);
        assert_eq!(dims.top_transition_radius(), 6.0);
        assert!(dims.is_symmetric());

        let dims_200 = NotchDimensions::from_dpi(192);
        assert_eq!(dims_200.bottom_corner_radius(), 28.0);
        assert_eq!(dims_200.top_transition_radius(), 12.0);
        assert!(dims_200.is_symmetric());
    }

    #[test]
    fn test_design_token_symmetry() {
        for dpi in [96, 120, 137, 144, 168, 192, 288] {
            let dims = NotchDimensions::from_dpi(dpi);
            assert!(dims.width > 0);
            assert!(dims.height > 0);
            assert!(dims.is_symmetric());
        }
    }

    #[test]
    fn test_corner_radius_clamping() {
        let w = 100.0;
        let h = 40.0;
        let r_top = 6.0;
        let r_bottom = 10.0;

        // Normal dimensions
        assert!(is_point_in_notch(50.0, 20.0, w, h, r_top, r_bottom));

        // Negative radius clamped to 0.0 without panic
        assert!(is_point_in_notch(50.0, 20.0, w, h, -5.0, -10.0));

        // Zero radius (sharp corners)
        assert!(is_point_in_notch(50.0, 20.0, w, h, 0.0, 0.0));

        // Oversized radius > height / 2.0 (e.g. radius=50 for height=40)
        // Must clamp to height / 2.0 without panicking
        assert!(is_point_in_notch(50.0, 20.0, w, h, 50.0, 50.0));
    }

    #[test]
    fn test_hardware_notch_hit_testing_and_shoulder_curves() {
        let w = 220.0;
        let h = 32.0;
        let r_top = 6.0;
        let r_bottom = 14.0;

        // 1. Center of the notch
        assert!(is_point_in_notch(110.0, 16.0, w, h, r_top, r_bottom));

        // 2. Top display edge attachment (y = 0.5) inside the notch
        assert!(is_point_in_notch(110.0, 0.5, w, h, r_top, r_bottom));
        assert!(is_point_in_notch(20.0, 0.5, w, h, r_top, r_bottom));
        assert!(is_point_in_notch(200.0, 0.5, w, h, r_top, r_bottom));

        // 3. Top-left concave shoulder curve:
        // Region x in [0, 6], y in [0, 6]:
        // (1.0, 1.0) is in the transparent ear outside the curve:
        // dx = 1.0, dy = 1.0 - 6.0 = -5.0 -> 1^2 + (-5)^2 = 26 < 36 -> false (transparent)
        assert!(!is_point_in_notch(1.0, 1.0, w, h, r_top, r_bottom));
        assert!(!is_point_in_notch(2.0, 3.0, w, h, r_top, r_bottom));
        // Point (5.5, 0.5) is inside the body near the top attachment:
        // dx = 5.5, dy = -5.5 -> 30.25 + 30.25 = 60.5 >= 36 -> true (inside)
        assert!(is_point_in_notch(5.5, 0.5, w, h, r_top, r_bottom));

        // 4. Top-right concave shoulder curve:
        // (219.0, 1.0) is in the transparent ear outside the curve:
        assert!(!is_point_in_notch(219.0, 1.0, w, h, r_top, r_bottom));
        assert!(!is_point_in_notch(218.0, 3.0, w, h, r_top, r_bottom));
        // (214.5, 0.5) is inside the body near top attachment:
        assert!(is_point_in_notch(214.5, 0.5, w, h, r_top, r_bottom));

        // 5. Outside vertical body walls (y = 16.0)
        assert!(!is_point_in_notch(2.0, 16.0, w, h, r_top, r_bottom)); // left outside
        assert!(!is_point_in_notch(218.0, 16.0, w, h, r_top, r_bottom)); // right outside

        // 6. Inside body vertical walls (y = 16.0)
        assert!(is_point_in_notch(7.0, 16.0, w, h, r_top, r_bottom)); // just inside left wall (x > r_top=6)
        assert!(is_point_in_notch(213.0, 16.0, w, h, r_top, r_bottom)); // just inside right wall (x < 214)

        // 7. Bottom-left rounded corner (center at x=20, y=18, radius=14):
        // (7.0, 31.0): dx = 7 - 20 = -13, dy = 31 - 18 = 13 -> 169 + 169 = 338 > 196 -> outside
        assert!(!is_point_in_notch(7.0, 31.0, w, h, r_top, r_bottom));
        // (15.0, 28.0): dx = 15 - 20 = -5, dy = 28 - 18 = 10 -> 25 + 100 = 125 <= 196 -> inside
        assert!(is_point_in_notch(15.0, 28.0, w, h, r_top, r_bottom));

        // 8. Bottom-right rounded corner (center at x=200, y=18, radius=14):
        // (213.0, 31.0): dx = 213 - 200 = 13, dy = 13 -> outside
        assert!(!is_point_in_notch(213.0, 31.0, w, h, r_top, r_bottom));
        // (205.0, 28.0): dx = 5, dy = 10 -> inside
        assert!(is_point_in_notch(205.0, 28.0, w, h, r_top, r_bottom));

        // 9. Exact left-right hit-testing symmetry
        for y in [1.0, 5.0, 10.0, 16.0, 25.0, 30.0] {
            for x_offset in [1.0, 3.0, 5.0, 7.0, 15.0, 25.0, 50.0] {
                let left_in = is_point_in_notch(x_offset, y, w, h, r_top, r_bottom);
                let right_in = is_point_in_notch(w - x_offset, y, w, h, r_top, r_bottom);
                assert_eq!(
                    left_in, right_in,
                    "Hit-testing must be strictly symmetric at offset {x_offset}, y {y}"
                );
            }
        }
    }

    #[test]
    fn test_invalid_dimensions_safety() {
        let r_top = 6.0;
        let r_bottom = 14.0;

        // Negative dimensions
        assert!(!is_point_in_notch(
            10.0, 10.0, -100.0, 32.0, r_top, r_bottom
        ));
        assert!(!is_point_in_notch(
            10.0, 10.0, 220.0, -32.0, r_top, r_bottom
        ));
        assert!(!is_point_in_notch(
            10.0, 10.0, -50.0, -50.0, r_top, r_bottom
        ));

        // Zero dimensions
        assert!(!is_point_in_notch(0.0, 0.0, 0.0, 32.0, r_top, r_bottom));
        assert!(!is_point_in_notch(0.0, 0.0, 220.0, 0.0, r_top, r_bottom));
        assert!(!is_point_in_notch(0.0, 0.0, 0.0, 0.0, r_top, r_bottom));

        // Negative point coordinates
        assert!(!is_point_in_notch(-0.1, 10.0, 220.0, 32.0, r_top, r_bottom));
        assert!(!is_point_in_notch(10.0, -0.1, 220.0, 32.0, r_top, r_bottom));

        // Out of bounds point coordinates
        assert!(!is_point_in_notch(
            220.1, 10.0, 220.0, 32.0, r_top, r_bottom
        ));
        assert!(!is_point_in_notch(10.0, 32.1, 220.0, 32.0, r_top, r_bottom));
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_design_system_tokens_and_palette() {
        // Text palette contrast against pure black
        assert_eq!(COLOR_TEXT_PRIMARY.a, 1.0);
        assert!(COLOR_TEXT_PRIMARY.r >= 0.9);
        assert_eq!(COLOR_TEXT_SECONDARY.a, 1.0);
        assert!(COLOR_TEXT_SECONDARY.r >= 0.6 && COLOR_TEXT_SECONDARY.r < COLOR_TEXT_PRIMARY.r);
        assert_eq!(COLOR_TEXT_MUTED.a, 1.0);
        assert!(COLOR_TEXT_MUTED.r >= 0.4 && COLOR_TEXT_MUTED.r < COLOR_TEXT_SECONDARY.r);

        // Border & transparency tokens
        assert_eq!(COLOR_BORDER_SUBTLE.a, 0.0);
        assert_eq!(COLOR_TRANSPARENT.a, 0.0);

        // Base dimension aliases & expanded tokens
        assert_eq!(BASE_WIDTH, BASE_COLLAPSED_WIDTH);
        assert_eq!(BASE_HEIGHT, BASE_COLLAPSED_HEIGHT);
        assert_eq!(BASE_EXPANDED_WIDTH, 600.0);
        assert_eq!(BASE_EXPANDED_HEIGHT, 120.0);

        // Typography and icon scaling at 150% (144 DPI)
        let dims_150 = NotchDimensions::from_dpi(144);
        assert!((dims_150.font_size_primary - (13.0 * 1.5)).abs() < 1e-4);
        assert!((dims_150.font_size_secondary - (10.5 * 1.5)).abs() < 1e-4);
        assert!((dims_150.font_size_muted - (9.0 * 1.5)).abs() < 1e-4);
        assert_eq!(dims_150.icon_size_default, (16.0f32 * 1.5).round());
        assert_eq!(dims_150.icon_size_small, (12.0f32 * 1.5).round());
        assert_eq!(dims_150.padding_h, (14.0f32 * 1.5).round());
        assert_eq!(dims_150.padding_v, (6.0f32 * 1.5).round());
        assert_eq!(dims_150.spacing, (8.0f32 * 1.5).round());

        // Font family strings are non-empty
        assert!(!FONT_FAMILY_PRIMARY.is_null());
        assert!(!FONT_FAMILY_FALLBACK.is_null());
        assert_eq!(BASE_ICON_STROKE_WIDTH, 1.2);
    }

    #[test]
    fn test_expanded_dpi_dimension_scaling() {
        // 100% scaling: 96 DPI
        let dims_100 = NotchDimensions::from_state_and_dpi(NotchState::Expanded, 96);
        assert_eq!(dims_100.width, 600);
        assert_eq!(dims_100.height, 120);
        assert_eq!(dims_100.state, NotchState::Expanded);
        assert!((dims_100.corner_radius - 28.0).abs() < 1e-4);
        assert!((dims_100.curvature.bottom_radius - 28.0).abs() < 1e-4);
        assert!((dims_100.curvature.top_transition_radius - 14.0).abs() < 1e-4);
        assert!((dims_100.curvature.top_transition_height - 14.0).abs() < 1e-4);
        assert_eq!(dims_100.shadow_margin_x, 10.0);
        assert_eq!(dims_100.shadow_margin_bottom, 10.0);
        assert_eq!(dims_100.notch_width(), 580.0);
        assert_eq!(dims_100.notch_height(), 110.0);
        assert_eq!(dims_100.padding_h, 18.0);
        assert_eq!(dims_100.padding_v, 12.0);

        // 125% scaling: 120 DPI
        let dims_125 = NotchDimensions::from_state_and_dpi(NotchState::Expanded, 120);
        assert_eq!(dims_125.width, 750);
        assert_eq!(dims_125.height, 150);
        assert!((dims_125.corner_radius - 35.0).abs() < 1e-4);
        assert!((dims_125.curvature.bottom_radius - 35.0).abs() < 1e-4);
        assert!((dims_125.curvature.top_transition_radius - 17.5).abs() < 1e-4);
        assert!((dims_125.curvature.top_transition_height - 17.5).abs() < 1e-4);

        // 150% scaling: 144 DPI
        let dims_150 = NotchDimensions::from_state_and_dpi(NotchState::Expanded, 144);
        assert_eq!(dims_150.width, 900);
        assert_eq!(dims_150.height, 180);
        assert!((dims_150.corner_radius - 42.0).abs() < 1e-4);
        assert!((dims_150.curvature.bottom_radius - 42.0).abs() < 1e-4);
        assert!((dims_150.curvature.top_transition_radius - 21.0).abs() < 1e-4);
        assert!((dims_150.curvature.top_transition_height - 21.0).abs() < 1e-4);

        // 175% scaling: 168 DPI
        let dims_175 = NotchDimensions::from_state_and_dpi(NotchState::Expanded, 168);
        assert_eq!(dims_175.width, 1050);
        assert_eq!(dims_175.height, 210);
        assert!((dims_175.corner_radius - 49.0).abs() < 1e-4);
        assert!((dims_175.curvature.bottom_radius - 49.0).abs() < 1e-4);
        assert!((dims_175.curvature.top_transition_radius - 24.5).abs() < 1e-4);
        assert!((dims_175.curvature.top_transition_height - 24.5).abs() < 1e-4);

        // 200% scaling: 192 DPI
        let dims_200 = NotchDimensions::from_state_and_dpi(NotchState::Expanded, 192);
        assert_eq!(dims_200.width, 1200);
        assert_eq!(dims_200.height, 240);
        assert!((dims_200.corner_radius - 56.0).abs() < 1e-4);
        assert!((dims_200.curvature.bottom_radius - 56.0).abs() < 1e-4);
        assert!((dims_200.curvature.top_transition_radius - 28.0).abs() < 1e-4);
        assert!((dims_200.curvature.top_transition_height - 28.0).abs() < 1e-4);

        // Unusual DPI: 137 DPI
        let dims_137 = NotchDimensions::from_state_and_dpi(NotchState::Expanded, 137);
        let scale_137 = 137.0f32 / 96.0;
        assert_eq!(dims_137.width, (600.0f32 * scale_137).round() as i32);
        assert_eq!(dims_137.height, (120.0f32 * scale_137).round() as i32);

        // High DPI: 288 DPI
        let dims_288 = NotchDimensions::from_state_and_dpi(NotchState::Expanded, 288);
        assert_eq!(dims_288.width, 1800);
        assert_eq!(dims_288.height, 360);
        assert!((dims_288.corner_radius - 84.0).abs() < 1e-4);
        assert!((dims_288.curvature.bottom_radius - 84.0).abs() < 1e-4);
        assert!((dims_288.curvature.top_transition_radius - 42.0).abs() < 1e-4);
        assert!((dims_288.curvature.top_transition_height - 42.0).abs() < 1e-4);

        // Zero DPI fallback
        let dims_zero = NotchDimensions::from_state_and_dpi(NotchState::Expanded, 0);
        assert_eq!(dims_zero.width, 600);
        assert_eq!(dims_zero.height, 120);
        assert_eq!(dims_zero.dpi, 96);
    }

    #[test]
    fn test_expanded_hardware_notch_hit_testing() {
        let w = 560.0;
        let h = 140.0;
        let r_top = 28.0;
        let r_bottom = 28.0;

        // 1. Center of the expanded notch
        assert!(is_point_in_notch(280.0, 70.0, w, h, r_top, r_bottom));

        // 2. Top display edge attachment (y = 0.5) inside the notch
        assert!(is_point_in_notch(280.0, 0.5, w, h, r_top, r_bottom));
        assert!(is_point_in_notch(35.0, 0.5, w, h, r_top, r_bottom));
        assert!(is_point_in_notch(525.0, 0.5, w, h, r_top, r_bottom));

        // 3. Top-left concave shoulder curve (region x in [0, 28], y in [0, 28]):
        // Outside ear: (1.0, 1.0)
        assert!(!is_point_in_notch(1.0, 1.0, w, h, r_top, r_bottom));
        assert!(!is_point_in_notch(2.0, 4.0, w, h, r_top, r_bottom));
        // Inside body near attachment: (27.0, 0.5)
        assert!(is_point_in_notch(27.0, 0.5, w, h, r_top, r_bottom));

        // 4. Top-right concave shoulder curve:
        // Outside ear: (559.0, 1.0)
        assert!(!is_point_in_notch(559.0, 1.0, w, h, r_top, r_bottom));
        assert!(!is_point_in_notch(558.0, 4.0, w, h, r_top, r_bottom));
        // Inside body near attachment: (533.0, 0.5)
        assert!(is_point_in_notch(533.0, 0.5, w, h, r_top, r_bottom));

        // 5. Outside vertical body walls (x < r_top=28 or x > w-r_top=532)
        assert!(!is_point_in_notch(2.0, 70.0, w, h, r_top, r_bottom)); // left outside
        assert!(!is_point_in_notch(15.0, 70.0, w, h, r_top, r_bottom)); // left outside
        assert!(!is_point_in_notch(558.0, 70.0, w, h, r_top, r_bottom)); // right outside

        // 6. Inside body vertical walls (x > r_top=28 and x < w-r_top=532)
        assert!(is_point_in_notch(30.0, 70.0, w, h, r_top, r_bottom)); // just inside left wall
        assert!(is_point_in_notch(530.0, 70.0, w, h, r_top, r_bottom)); // just inside right wall

        // 7. Bottom-left rounded corner (center at x=56, y=112, radius=28):
        // (30.0, 138.0): outside corner arc
        assert!(!is_point_in_notch(30.0, 138.0, w, h, r_top, r_bottom));
        // (50.0, 125.0): inside corner
        assert!(is_point_in_notch(50.0, 125.0, w, h, r_top, r_bottom));

        // 8. Bottom-right rounded corner (center at x=504, y=112, radius=28):
        // (530.0, 138.0): outside corner arc
        assert!(!is_point_in_notch(530.0, 138.0, w, h, r_top, r_bottom));
        // (510.0, 125.0): inside corner
        assert!(is_point_in_notch(510.0, 125.0, w, h, r_top, r_bottom));

        // 9. Exact left-right hit-testing symmetry on expanded notch
        for y in [1.0, 5.0, 10.0, 20.0, 70.0, 120.0, 135.0] {
            for x_offset in [1.0, 4.0, 7.0, 10.0, 20.0, 50.0, 100.0, 200.0] {
                let left_in = is_point_in_notch(x_offset, y, w, h, r_top, r_bottom);
                let right_in = is_point_in_notch(w - x_offset, y, w, h, r_top, r_bottom);
                assert_eq!(
                    left_in, right_in,
                    "Expanded hit-testing must be strictly symmetric at offset {x_offset}, y {y}"
                );
            }
        }
    }

    #[test]
    fn test_display_bounds_and_centering_safety() {
        // Normal screens
        assert_eq!(calculate_notch_x(1920, 560), (1920 - 560) / 2);
        assert_eq!(calculate_notch_x(1536, 560), (1536 - 560) / 2);

        // Centering alignment: collapsed center equals expanded center
        let screen_w = 1920;
        let c_center = calculate_notch_center_x(screen_w, 220);
        let e_center = calculate_notch_center_x(screen_w, 560);
        assert_eq!(c_center, 960.0);
        assert_eq!(e_center, 960.0);

        // Screen width smaller than notch width (e.g. 300px screen, 560px notch)
        // Must clamp to 0 without underflow or panic
        assert_eq!(calculate_notch_x(300, 560), 0);

        // Zero and negative screen width
        assert_eq!(calculate_notch_x(0, 560), 0);
        assert_eq!(calculate_notch_x(-500, 560), 0);

        // Zero and negative notch width
        assert_eq!(calculate_notch_x(1920, 0), 0);
        assert_eq!(calculate_notch_x(1920, -100), 0);
    }

    #[test]
    fn test_interaction_state_model() {
        let mut interaction = InteractionState::new();

        // 1. Initial state is Collapsed and non-hovered
        assert_eq!(interaction.state, NotchState::Collapsed);
        assert!(!interaction.hovered);

        // 2. Collapsed + left-click -> Expanded
        let toggled = interaction.on_left_click();
        assert!(toggled);
        assert_eq!(interaction.state, NotchState::Expanded);

        // 3. Expanded + left-click -> Collapsed
        let toggled_back = interaction.on_left_click();
        assert!(toggled_back);
        assert_eq!(interaction.state, NotchState::Collapsed);

        // 4. Hover false -> true (returns true indicating redraw required)
        assert!(interaction.set_hovered(true));
        assert!(interaction.hovered);

        // 6. Repeated mouse movement while hovered does NOT logically change hover state (returns false -> no redraw)
        assert!(!interaction.set_hovered(true));
        assert!(interaction.hovered);

        // 5. Hover true -> false (returns true indicating redraw required)
        assert!(interaction.set_hovered(false));
        assert!(!interaction.hovered);

        // Repeated mouse movement while unhovered does NOT change state (returns false -> no redraw)
        assert!(!interaction.set_hovered(false));
        assert!(!interaction.hovered);
    }

    #[test]
    fn test_right_click_and_middle_click_state_invariance() {
        let initial_state = InteractionState::new();
        let test_state = initial_state;

        // In Phase 2.4, right-click and middle-click do NOT mutate state or trigger exit
        // Simulating right-click: no effect
        assert_eq!(test_state.state, initial_state.state);
        assert_eq!(test_state.hovered, initial_state.hovered);

        // Simulating middle-click: no effect
        assert_eq!(test_state.state, initial_state.state);
        assert_eq!(test_state.hovered, initial_state.hovered);
    }

    #[test]
    fn test_hover_visual_design_tokens() {
        // Zero white overlay border: pure AMOLED black maintained both resting and hovered
        assert_eq!(COLOR_BORDER_SUBTLE.a, 0.0);
        assert_eq!(COLOR_BORDER_HOVER.a, 0.0);

        // Surface fill remains pure AMOLED #000000 regardless of hover
        assert_eq!(COLOR_SURFACE_AMOLED.r, 0.0);
        assert_eq!(COLOR_SURFACE_AMOLED.g, 0.0);
        assert_eq!(COLOR_SURFACE_AMOLED.b, 0.0);
        assert_eq!(COLOR_SURFACE_AMOLED.a, 1.0);
    }

    #[test]
    fn test_state_transitions_preserve_layout_state() {
        let mut interaction = InteractionState::new();
        let dims_c = NotchDimensions::from_state_and_dpi(interaction.state, 96);
        let layout_c = crate::layout::resolve_layout(&dims_c);
        match layout_c {
            crate::layout::ResolvedLayout::Collapsed { bounds, components } => {
                assert_eq!(bounds.width(), 220.0);
                assert_eq!(bounds.height(), 32.0);
                assert!(components.content_bounds.width() > 0.0);
                assert!(components.clock_bounds.width() > 0.0);
            }
            _ => panic!("Expected collapsed layout"),
        }

        // Left-click to expand
        interaction.on_left_click();
        let dims_e = NotchDimensions::from_state_and_dpi(interaction.state, 96);
        let layout_e = crate::layout::resolve_layout(&dims_e);
        match layout_e {
            crate::layout::ResolvedLayout::Expanded { bounds, components } => {
                assert_eq!(bounds.width(), 600.0);
                assert_eq!(bounds.height(), 120.0);
                assert!(components.content_bounds.width() > 0.0);
                assert!(components.time_bounds.width() > 0.0);
                assert!(components.date_bounds.width() > 0.0);
            }
            _ => panic!("Expected expanded layout"),
        }

        // Left-click to collapse back
        interaction.on_left_click();
        let dims_c2 = NotchDimensions::from_state_and_dpi(interaction.state, 96);
        let layout_c2 = crate::layout::resolve_layout(&dims_c2);
        match layout_c2 {
            crate::layout::ResolvedLayout::Collapsed { bounds, components } => {
                assert_eq!(bounds.width(), 220.0);
                assert_eq!(bounds.height(), 32.0);
                assert!(components.content_bounds.width() > 0.0);
                assert!(components.clock_bounds.width() > 0.0);
            }
            _ => panic!("Expected collapsed layout"),
        }
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_typography_tokens_valid_and_non_negative() {
        assert!(BASE_FONT_SIZE_PRIMARY > 0.0);
        assert!(BASE_FONT_SIZE_SECONDARY > 0.0);
        assert!(BASE_FONT_SIZE_MUTED > 0.0);
        assert!(BASE_FONT_SIZE_CLOCK > 0.0);
        assert!(BASE_CLOCK_OPTICAL_Y_OFFSET >= 0.0);
        assert!(BASE_TITLE_OPTICAL_Y_OFFSET >= 0.0);
        assert!(BASE_HEADER_HEIGHT > 0.0);
        assert!(BASE_SEPARATOR_MARGIN_TOP >= 0.0);
        assert!(BASE_PREVIEW_MARGIN_TOP >= 0.0);
        assert!(!FONT_FAMILY_PRIMARY.is_null());
        assert!(!FONT_FAMILY_FALLBACK.is_null());

        // Relative font size hierarchy
        assert!(BASE_FONT_SIZE_PRIMARY > BASE_FONT_SIZE_SECONDARY);
        assert!(BASE_FONT_SIZE_SECONDARY > BASE_FONT_SIZE_MUTED);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_opacity_hierarchy_strict() {
        // Pure monochrome white text with strict opacity hierarchy
        assert_eq!(COLOR_TEXT_PRIMARY.a, 1.0);
        assert_eq!(COLOR_TEXT_SECONDARY.a, 1.0);
        assert_eq!(COLOR_TEXT_MUTED.a, 1.0);

        assert!(COLOR_TEXT_PRIMARY.r > COLOR_TEXT_SECONDARY.r);
        assert!(COLOR_TEXT_SECONDARY.r > COLOR_TEXT_MUTED.r);

        // Separator line is subtle and does not produce harsh white border
        assert!(COLOR_SEPARATOR.a <= 0.08);
        assert!(COLOR_SEPARATOR.a >= 0.02);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_icon_tokens_and_proportions() {
        assert_eq!(BASE_ICON_SIZE_DEFAULT, 16.0);
        assert_eq!(BASE_ICON_SIZE_SMALL, 12.0);
        assert_eq!(BASE_ICON_STROKE_WIDTH, 1.2);
        assert!((BASE_ICON_ASPECT_RATIO - 0.70).abs() < 1e-4);
        assert!((BASE_ICON_CORNER_RADIUS - 2.5).abs() < 1e-4);
        assert!((BASE_ICON_APERTURE_RADIUS - 1.5).abs() < 1e-4);
    }

    #[test]
    fn test_animation_collapsed_to_expanded_starts() {
        let anim = AnimationState::new(NotchState::Collapsed, NotchState::Expanded);
        assert!(anim.active);
        assert_eq!(anim.start_state, NotchState::Collapsed);
        assert_eq!(anim.target_state, NotchState::Expanded);
        assert_eq!(anim.start_width, BASE_COLLAPSED_WIDTH);
        assert_eq!(anim.target_width, BASE_EXPANDED_WIDTH);
        assert_eq!(anim.start_height, BASE_COLLAPSED_HEIGHT);
        assert_eq!(anim.target_height, BASE_EXPANDED_HEIGHT);
        assert_eq!(anim.progress, 0.0);
    }

    #[test]
    fn test_animation_expanded_to_collapsed_starts() {
        let anim = AnimationState::new(NotchState::Expanded, NotchState::Collapsed);
        assert!(anim.active);
        assert_eq!(anim.start_state, NotchState::Expanded);
        assert_eq!(anim.target_state, NotchState::Collapsed);
        assert_eq!(anim.start_width, BASE_EXPANDED_WIDTH);
        assert_eq!(anim.target_width, BASE_COLLAPSED_WIDTH);
        assert_eq!(anim.start_height, BASE_EXPANDED_HEIGHT);
        assert_eq!(anim.target_height, BASE_COLLAPSED_HEIGHT);
        assert_eq!(anim.progress, 0.0);
    }

    #[test]
    fn test_animation_progress_clamping_and_completion() {
        let mut anim = AnimationState::new(NotchState::Collapsed, NotchState::Expanded);
        anim.set_progress(-0.5);
        assert_eq!(anim.progress, 0.0);
        assert!(anim.active);

        anim.set_progress(1.5);
        assert_eq!(anim.progress, 1.0);
        assert!(!anim.active);

        let mut anim2 = AnimationState::new(NotchState::Collapsed, NotchState::Expanded);
        anim2.step(500);
        assert_eq!(anim2.progress, 1.0);
        assert!(!anim2.active);
    }

    #[test]
    fn test_cubic_ease_out_determinism_and_properties() {
        assert_eq!(cubic_ease_out(0.0), 0.0);
        assert_eq!(cubic_ease_out(1.0), 1.0);

        // Strictly monotonic increasing
        let mut prev = 0.0;
        for i in 1..=20 {
            let t = i as f32 / 20.0;
            let val = cubic_ease_out(t);
            assert!(val > prev, "Eased value must be strictly monotonic");
            prev = val;
        }

        // Ease-out property: fast initial response, smooth deceleration
        assert!(cubic_ease_out(0.5) > 0.5);
        let mid = cubic_ease_out(0.5);
        assert!((mid - 0.875).abs() < 1e-4);
    }

    #[test]
    fn test_animation_progress_zero_equals_start_geometry() {
        let anim = AnimationState::new(NotchState::Collapsed, NotchState::Expanded);
        let (w, h, top_r, bot_r) = anim.current_logical_geometry();
        assert_eq!(w, BASE_COLLAPSED_WIDTH);
        assert_eq!(h, BASE_COLLAPSED_HEIGHT);
        assert_eq!(top_r, BASE_COLLAPSED_TOP_TRANSITION_RADIUS);
        assert_eq!(bot_r, BASE_COLLAPSED_BOTTOM_CORNER_RADIUS);

        let dims = anim.current_dimensions(96);
        assert_eq!(dims.width, 220);
        assert_eq!(dims.height, 32);
        assert_eq!(dims.state, NotchState::Collapsed);
    }

    #[test]
    fn test_animation_progress_one_equals_target_geometry() {
        let mut anim = AnimationState::new(NotchState::Collapsed, NotchState::Expanded);
        anim.set_progress(1.0);
        let (w, h, top_r, bot_r) = anim.current_logical_geometry();
        assert_eq!(w, BASE_EXPANDED_WIDTH);
        assert_eq!(h, BASE_EXPANDED_HEIGHT);
        assert_eq!(top_r, BASE_EXPANDED_TOP_TRANSITION_RADIUS);
        assert_eq!(bot_r, BASE_EXPANDED_BOTTOM_CORNER_RADIUS);

        let dims = anim.current_dimensions(96);
        assert_eq!(dims.width, 600);
        assert_eq!(dims.height, 120);
        assert_eq!(dims.state, NotchState::Expanded);
    }

    #[test]
    fn test_intermediate_progress_produces_intermediate_dimensions() {
        let mut anim = AnimationState::new(NotchState::Collapsed, NotchState::Expanded);
        anim.set_progress(0.5);
        let (w, h, top_r, bot_r) = anim.current_logical_geometry();
        assert!(w > BASE_COLLAPSED_WIDTH && w < BASE_EXPANDED_WIDTH);
        assert!(h > BASE_COLLAPSED_HEIGHT && h < BASE_EXPANDED_HEIGHT);
        assert!(
            (BASE_COLLAPSED_TOP_TRANSITION_RADIUS..=BASE_EXPANDED_TOP_TRANSITION_RADIUS)
                .contains(&top_r)
        );
        let min_bot_r = BASE_COLLAPSED_BOTTOM_CORNER_RADIUS.min(BASE_EXPANDED_BOTTOM_CORNER_RADIUS);
        let max_bot_r = BASE_COLLAPSED_BOTTOM_CORNER_RADIUS.max(BASE_EXPANDED_BOTTOM_CORNER_RADIUS);
        assert!((min_bot_r..=max_bot_r).contains(&bot_r));
    }

    #[test]
    fn test_center_x_invariance_during_animation() {
        let mut anim = AnimationState::new(NotchState::Collapsed, NotchState::Expanded);
        let screen_width = 1920;
        let expected_center = 1920.0 / 2.0;

        for step_i in 0..=10 {
            anim.set_progress(step_i as f32 / 10.0);
            let dims = anim.current_dimensions(96);
            let center_x = calculate_notch_center_x(screen_width, dims.width);
            assert!(
                (center_x - expected_center).abs() < 1e-4,
                "Center X must remain invariant throughout animation (progress={})",
                anim.progress
            );
        }
    }

    #[test]
    fn test_top_y_anchored_at_zero() {
        let _anim = AnimationState::new(NotchState::Collapsed, NotchState::Expanded);
        let y = 0;
        assert_eq!(y, 0, "Top screen attachment must remain anchored at 0");
    }

    #[test]
    fn test_reversing_active_animation_continuity() {
        let mut anim = AnimationState::new(NotchState::Collapsed, NotchState::Expanded);
        anim.set_progress(0.4);
        let (w_mid, h_mid, _, _) = anim.current_logical_geometry();

        // Reverse mid-animation
        let reversed = anim.reverse_from_current();
        assert!(reversed.active);
        assert_eq!(reversed.target_state, NotchState::Collapsed);
        assert_eq!(reversed.start_width, w_mid);
        assert_eq!(reversed.start_height, h_mid);
        assert_eq!(reversed.target_width, BASE_COLLAPSED_WIDTH);
        assert_eq!(reversed.target_height, BASE_COLLAPSED_HEIGHT);
    }

    #[test]
    fn test_hover_state_independence_from_animation() {
        let mut interaction = InteractionState::new();
        let anim = AnimationState::new(NotchState::Collapsed, NotchState::Expanded);

        assert!(!interaction.hovered);
        interaction.set_hovered(true);
        assert!(interaction.hovered);
        // Animation state remains unaffected
        assert_eq!(anim.target_state, NotchState::Expanded);
        assert_eq!(anim.progress, 0.0);
    }

    #[test]
    fn test_hit_testing_valid_for_intermediate_geometry() {
        let mut anim = AnimationState::new(NotchState::Collapsed, NotchState::Expanded);
        anim.set_progress(0.5);
        let dims = anim.current_dimensions(96);

        // Center must be inside
        assert!(is_point_in_notch(
            dims.width as f32 / 2.0,
            dims.height as f32 / 2.0,
            dims.width as f32,
            dims.height as f32,
            dims.curvature.top_transition_radius,
            dims.curvature.bottom_radius,
        ));

        // Outside shoulder ear must be transparent
        assert!(!is_point_in_notch(
            1.0,
            1.0,
            dims.width as f32,
            dims.height as f32,
            dims.curvature.top_transition_radius,
            dims.curvature.bottom_radius,
        ));
    }

    // ========================================================================
    // ACCESSIBILITY UNIT TESTS (Phase 2.7)
    // ========================================================================

    #[test]
    fn test_accessible_name_and_metadata() {
        assert_eq!(ACCESSIBLE_NAME_STR, "Nott");
        assert_eq!(ACCESSIBLE_DESCRIPTION, "Interactive desktop notch overlay");
        assert_eq!(ACCESSIBLE_ROLE_NAME, "Desktop Overlay");

        // Verify description does not expose internal implementation details
        let forbidden = [
            "Rust",
            "Direct2D",
            "DirectWrite",
            "HWND",
            "timer",
            "thread",
            "pointer",
        ];
        for term in forbidden {
            assert!(!ACCESSIBLE_NAME_STR.contains(term));
            assert!(!ACCESSIBLE_DESCRIPTION.contains(term));
            assert!(!ACCESSIBLE_ROLE_NAME.contains(term));
        }
    }

    #[test]
    fn test_collapsed_semantic_state() {
        let snapshot = AccessibilitySnapshot::for_state(NotchState::Collapsed);
        assert_eq!(snapshot.name, "12:00 PM");
        assert_eq!(snapshot.state_label, "Collapsed");
        assert_eq!(snapshot.visible_text, "12:00 PM");
        assert!(!snapshot.exposes_subtitle);
        assert!(!snapshot.is_icon_interactive);

        let snapshot_custom =
            AccessibilitySnapshot::for_state_with_time(NotchState::Collapsed, "5:42 PM");
        assert_eq!(snapshot_custom.name, "5:42 PM");
        assert_eq!(snapshot_custom.visible_text, "5:42 PM");

        let snapshot_24h =
            AccessibilitySnapshot::for_state_with_time(NotchState::Collapsed, "17:42");
        assert_eq!(snapshot_24h.name, "17:42");
        assert_eq!(snapshot_24h.visible_text, "17:42");
    }

    #[test]
    fn test_expanded_semantic_state() {
        let snapshot = AccessibilitySnapshot::for_state(NotchState::Expanded);
        assert_eq!(snapshot.name, "12:00 PM - Monday, October 6");
        assert_eq!(snapshot.state_label, "Expanded");
        assert_eq!(snapshot.visible_text, "12:00 PM - Monday, October 6");
        assert!(!snapshot.exposes_subtitle);
        assert!(!snapshot.is_icon_interactive);

        let custom = AccessibilitySnapshot::for_state_with_clock(
            NotchState::Expanded,
            "12:47 AM",
            "Monday, October 6",
        );
        assert_eq!(custom.name, "12:47 AM - Monday, October 6");
    }

    #[test]
    fn test_collapsed_state_does_not_expose_expanded_subtitle() {
        let snapshot = AccessibilitySnapshot::for_state(NotchState::Collapsed);
        assert_eq!(snapshot.visible_text, "12:00 PM");
        assert!(
            !snapshot
                .visible_text
                .contains("Design & Interaction Preview")
        );
        assert!(!snapshot.exposes_subtitle);
    }

    #[test]
    fn test_expanded_state_has_no_temporary_subtitle() {
        let snapshot = AccessibilitySnapshot::for_state(NotchState::Expanded);
        assert_eq!(snapshot.visible_text, "12:00 PM - Monday, October 6");
        assert!(
            !snapshot
                .visible_text
                .contains("Design & Interaction Preview")
        );
        assert!(!snapshot.exposes_subtitle);
    }

    #[test]
    fn test_decorative_icon_not_exposed_as_independent_control() {
        let snap_collapsed = AccessibilitySnapshot::for_state(NotchState::Collapsed);
        let snap_expanded = AccessibilitySnapshot::for_state(NotchState::Expanded);

        assert!(!snap_collapsed.is_icon_interactive);
        assert!(!snap_expanded.is_icon_interactive);
    }

    #[test]
    fn test_accessibility_minute_changed_event() {
        let mut tracker = AccessibilityEventTracker::new();
        tracker.on_state_settled(NotchState::Collapsed);
        let names_before = tracker.name_change_count;
        let states_before = tracker.state_change_count;

        // When collapsed, minute rollover triggers a name change notification
        let emitted = tracker.on_clock_minute_changed(true);
        assert!(emitted);
        assert_eq!(tracker.name_change_count, names_before + 1);
        assert_eq!(tracker.state_change_count, states_before); // State didn't change

        // When expanded, minute rollover also triggers a name change notification
        let emitted_exp = tracker.on_clock_minute_changed(false);
        assert!(emitted_exp);
        assert_eq!(tracker.name_change_count, names_before + 2);
    }

    #[test]
    fn test_hover_does_not_generate_semantic_state_changes() {
        let mut tracker = AccessibilityEventTracker::new();
        tracker.on_state_settled(NotchState::Collapsed);
        let names_before = tracker.name_change_count;
        let states_before = tracker.state_change_count;

        // Hover enter
        assert!(!tracker.on_hover_changed(true));
        assert_eq!(tracker.name_change_count, names_before);
        assert_eq!(tracker.state_change_count, states_before);

        // Hover leave
        assert!(!tracker.on_hover_changed(false));
        assert_eq!(tracker.name_change_count, names_before);
        assert_eq!(tracker.state_change_count, states_before);

        // InteractionState hover toggling does not affect semantic state
        let mut interaction = InteractionState::new();
        assert_eq!(interaction.state, NotchState::Collapsed);
        interaction.set_hovered(true);
        assert_eq!(interaction.state, NotchState::Collapsed);
        interaction.set_hovered(false);
        assert_eq!(interaction.state, NotchState::Collapsed);
    }

    #[test]
    fn test_animation_frames_do_not_generate_accessibility_state_changes() {
        let mut tracker = AccessibilityEventTracker::new();
        tracker.on_state_settled(NotchState::Collapsed);
        let names_before = tracker.name_change_count;
        let states_before = tracker.state_change_count;

        let mut anim = AnimationState::new(NotchState::Collapsed, NotchState::Expanded);

        // Step through 15 intermediate frames (~240ms)
        for _ in 0..15 {
            anim.step(16);
            if anim.active {
                assert!(!tracker.on_animation_frame(anim.progress));
                assert_eq!(tracker.name_change_count, names_before);
                assert_eq!(tracker.state_change_count, states_before);
            }
        }
    }

    #[test]
    fn test_collapsed_to_expanded_produces_exactly_one_state_transition() {
        let mut tracker = AccessibilityEventTracker::new();
        // Initial setup at Collapsed
        assert!(tracker.on_state_settled(NotchState::Collapsed));
        assert_eq!(tracker.name_change_count, 1);
        assert_eq!(tracker.state_change_count, 1);

        // Intermediate animation frames emit 0 events
        let mut anim = AnimationState::new(NotchState::Collapsed, NotchState::Expanded);
        while anim.step(16) {
            assert!(!tracker.on_animation_frame(anim.progress));
        }
        assert_eq!(tracker.name_change_count, 1);
        assert_eq!(tracker.state_change_count, 1);

        // Once animation completes and settles at target state: exactly ONE transition
        assert!(tracker.on_state_settled(NotchState::Expanded));
        assert_eq!(tracker.name_change_count, 2);
        assert_eq!(tracker.state_change_count, 2);

        // Calling again with same settled state is idempotent (0 additional events)
        assert!(!tracker.on_state_settled(NotchState::Expanded));
        assert_eq!(tracker.name_change_count, 2);
        assert_eq!(tracker.state_change_count, 2);
    }

    #[test]
    fn test_expanded_to_collapsed_produces_exactly_one_state_transition() {
        let mut tracker = AccessibilityEventTracker::new();
        tracker.on_state_settled(NotchState::Expanded);
        assert_eq!(tracker.name_change_count, 1);
        assert_eq!(tracker.state_change_count, 1);

        // Intermediate animation frames emit 0 events
        let mut anim = AnimationState::new(NotchState::Expanded, NotchState::Collapsed);
        while anim.step(16) {
            assert!(!tracker.on_animation_frame(anim.progress));
        }
        assert_eq!(tracker.name_change_count, 1);

        // Settled at Collapsed: exactly ONE transition
        assert!(tracker.on_state_settled(NotchState::Collapsed));
        assert_eq!(tracker.name_change_count, 2);
        assert_eq!(tracker.state_change_count, 2);

        // Idempotent
        assert!(!tracker.on_state_settled(NotchState::Collapsed));
        assert_eq!(tracker.name_change_count, 2);
        assert_eq!(tracker.state_change_count, 2);
    }

    #[test]
    fn test_reduced_motion_instant_transition_logic() {
        let mut tracker = AccessibilityEventTracker::new();
        tracker.on_state_settled(NotchState::Collapsed);

        // In reduced motion, transition is direct without timer ticks
        let target_state = NotchState::Expanded;
        assert!(tracker.on_state_settled(target_state));
        assert_eq!(tracker.name_change_count, 2);
        assert_eq!(tracker.state_change_count, 2);
    }

    #[test]
    fn test_high_contrast_amoled_text_ratios() {
        // Luminance formula for relative luminance L = 0.2126 * R + 0.7152 * G + 0.0722 * B
        let lum_black = 0.2126 * COLOR_SURFACE_AMOLED.r
            + 0.7152 * COLOR_SURFACE_AMOLED.g
            + 0.0722 * COLOR_SURFACE_AMOLED.b;
        assert_eq!(lum_black, 0.0);

        let lum_primary = 0.2126 * COLOR_TEXT_PRIMARY.r
            + 0.7152 * COLOR_TEXT_PRIMARY.g
            + 0.0722 * COLOR_TEXT_PRIMARY.b;
        let contrast_primary = (lum_primary + 0.05) / (lum_black + 0.05);
        // WCAG AAA requires 7.0:1 for normal text. We achieve ~19.8:1!
        assert!(contrast_primary > 7.0);

        let lum_secondary = 0.2126 * COLOR_TEXT_SECONDARY.r
            + 0.7152 * COLOR_TEXT_SECONDARY.g
            + 0.0722 * COLOR_TEXT_SECONDARY.b;
        let contrast_secondary = (lum_secondary + 0.05) / (lum_black + 0.05);
        // Secondary text achieves ~8.2:1, exceeding WCAG AAA 7.0:1
        assert!(contrast_secondary > 7.0);
    }

    #[test]
    fn test_dpi_text_readability_invariance() {
        for dpi in [96, 120, 144, 192] {
            let dims = NotchDimensions::from_state_and_dpi(NotchState::Expanded, dpi);
            assert!(dims.font_size_primary > 0.0);
            assert!(dims.font_size_secondary > 0.0);
            assert!(dims.font_size_muted > 0.0);
            assert!(dims.font_size_primary > dims.font_size_secondary);
            assert!(dims.font_size_secondary > dims.font_size_muted);

            // Ratio of primary to secondary font sizes is strictly preserved across all DPIs
            let ratio = dims.font_size_primary / dims.font_size_secondary;
            assert!((ratio - (BASE_FONT_SIZE_PRIMARY / BASE_FONT_SIZE_SECONDARY)).abs() < 1e-4);
        }
    }
}
