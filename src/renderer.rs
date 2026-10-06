use std::cell::{Cell, RefCell};
use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_DRAW_TEXT_OPTIONS_NONE,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1CreateFactory,
    ID2D1DCRenderTarget, ID2D1Factory, ID2D1PathGeometry, ID2D1RenderTarget,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_REGULAR, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER, DWriteCreateFactory,
    IDWriteFactory, IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HBITMAP,
    HDC, HGDIOBJ, RGBQUAD, ReleaseDC, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow};
use windows::core::{Error, Result};

use crate::clock::ClockDateState;
use crate::config::{
    COLOR_BORDER_HOVER, COLOR_TEXT_PRIMARY, COLOR_TEXT_SECONDARY, COLOR_TRANSPARENT,
    FONT_FAMILY_FALLBACK, FONT_FAMILY_PRIMARY, NOTCH_BG_COLOR, NOTCH_BORDER_COLOR, NotchDimensions,
};
use crate::layout::{
    CollapsedLayout, ExpandedLayout, ResolvedLayout, resolve_collapsed_layout, resolve_layout,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D_POINT_2F {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct D2D1_BEZIER_SEGMENT {
    pub point1: D2D_POINT_2F,
    pub point2: D2D_POINT_2F,
    pub point3: D2D_POINT_2F,
}

pub struct GeometrySinkHelper {
    raw: *mut std::ffi::c_void,
    vtable: *const usize,
}

impl GeometrySinkHelper {
    pub unsafe fn from_raw(raw: *mut std::ffi::c_void) -> Self {
        unsafe {
            let vtable = *(raw as *const *const usize);
            Self { raw, vtable }
        }
    }

    pub unsafe fn begin_figure(&self, start: D2D_POINT_2F) {
        type FnBeginFigure = unsafe extern "system" fn(*mut std::ffi::c_void, D2D_POINT_2F, u32);
        unsafe {
            let func: FnBeginFigure = std::mem::transmute(*self.vtable.add(5));
            func(self.raw, start, 0); // 0 = D2D1_FIGURE_BEGIN_FILLED
        }
    }

    pub unsafe fn add_line(&self, point: D2D_POINT_2F) {
        type FnAddLine = unsafe extern "system" fn(*mut std::ffi::c_void, D2D_POINT_2F);
        unsafe {
            let func: FnAddLine = std::mem::transmute(*self.vtable.add(10));
            func(self.raw, point);
        }
    }

    pub unsafe fn add_bezier(&self, bezier: &D2D1_BEZIER_SEGMENT) {
        type FnAddBezier =
            unsafe extern "system" fn(*mut std::ffi::c_void, *const D2D1_BEZIER_SEGMENT);
        unsafe {
            let func: FnAddBezier = std::mem::transmute(*self.vtable.add(11));
            func(self.raw, bezier);
        }
    }

    pub unsafe fn end_figure(&self) {
        type FnEndFigure = unsafe extern "system" fn(*mut std::ffi::c_void, u32);
        unsafe {
            let func: FnEndFigure = std::mem::transmute(*self.vtable.add(8));
            func(self.raw, 1); // 1 = D2D1_FIGURE_END_CLOSED
        }
    }

    pub unsafe fn end_figure_open(&self) {
        type FnEndFigure = unsafe extern "system" fn(*mut std::ffi::c_void, u32);
        unsafe {
            let func: FnEndFigure = std::mem::transmute(*self.vtable.add(8));
            func(self.raw, 0); // 0 = D2D1_FIGURE_END_OPEN
        }
    }

    pub unsafe fn close(&self) -> Result<()> {
        type FnClose = unsafe extern "system" fn(*mut std::ffi::c_void) -> windows::core::HRESULT;
        unsafe {
            let func: FnClose = std::mem::transmute(*self.vtable.add(9));
            func(self.raw).ok()
        }
    }
}

pub struct Renderer {
    d2d_factory: ID2D1Factory,
    dwrite_factory: IDWriteFactory,
    cached_screen_dc: Cell<HDC>,
    cached_mem_dc: Cell<HDC>,
    cached_dib: Cell<HBITMAP>,
    cached_bits: Cell<*mut std::ffi::c_void>,
    cached_old_bmp: Cell<HGDIOBJ>,
    cached_rt: RefCell<Option<ID2D1DCRenderTarget>>,
    cached_capacity_w: Cell<i32>,
    cached_capacity_h: Cell<i32>,
    scratch_a: RefCell<Vec<u8>>,
    scratch_b: RefCell<Vec<u8>>,
}

impl Renderer {
    /// Creates a new Renderer instance initializing Direct2D and DirectWrite factories.
    pub fn new() -> Result<Self> {
        let d2d_factory: ID2D1Factory =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        let dwrite_factory: IDWriteFactory =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        Ok(Self {
            d2d_factory,
            dwrite_factory,
            cached_screen_dc: Cell::new(HDC::default()),
            cached_mem_dc: Cell::new(HDC::default()),
            cached_dib: Cell::new(HBITMAP::default()),
            cached_bits: Cell::new(std::ptr::null_mut()),
            cached_old_bmp: Cell::new(HGDIOBJ::default()),
            cached_rt: RefCell::new(None),
            cached_capacity_w: Cell::new(0),
            cached_capacity_h: Cell::new(0),
            scratch_a: RefCell::new(Vec::new()),
            scratch_b: RefCell::new(Vec::new()),
        })
    }

    /// Releases cached GDI surface and Direct2D render target resources.
    fn cleanup_cached_resources(&self) {
        unsafe {
            *self.cached_rt.borrow_mut() = None;
            let mem_dc = self.cached_mem_dc.get();
            if !mem_dc.is_invalid() {
                let old_bmp = self.cached_old_bmp.get();
                if !old_bmp.is_invalid() {
                    let _ = SelectObject(mem_dc, old_bmp);
                    self.cached_old_bmp.set(HGDIOBJ::default());
                }
                let dib = self.cached_dib.get();
                if !dib.is_invalid() {
                    let _ = DeleteObject(dib.into());
                    self.cached_dib.set(HBITMAP::default());
                }
                let _ = DeleteDC(mem_dc);
                self.cached_mem_dc.set(HDC::default());
            }
            let screen_dc = self.cached_screen_dc.get();
            if !screen_dc.is_invalid() {
                let _ = ReleaseDC(None, screen_dc);
                self.cached_screen_dc.set(HDC::default());
            }
            self.cached_bits.set(std::ptr::null_mut());
            self.cached_capacity_w.set(0);
            self.cached_capacity_h.set(0);
        }
    }

    /// Pre-allocates and caches high-performance GDI and Direct2D surfaces to guarantee
    /// zero-allocation, 60+ FPS frame rendering without stutter.
    fn ensure_buffer(&self, req_w: i32, req_h: i32) -> Result<()> {
        if self.cached_capacity_w.get() >= req_w
            && self.cached_capacity_h.get() >= req_h
            && self.cached_rt.borrow().is_some()
        {
            return Ok(());
        }

        let alloc_w = req_w.max(1200);
        let alloc_h = req_h.max(600);

        unsafe {
            self.cleanup_cached_resources();

            let screen_dc = GetDC(None);
            if screen_dc.is_invalid() {
                return Err(Error::from_thread());
            }

            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            if mem_dc.is_invalid() {
                let _ = ReleaseDC(None, screen_dc);
                return Err(Error::from_thread());
            }

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: alloc_w,
                    biHeight: -alloc_h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };

            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let dib_result =
                CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0);
            let dib = match dib_result {
                Ok(hbitmap) => {
                    if hbitmap.is_invalid() || bits.is_null() {
                        let _ = DeleteDC(mem_dc);
                        let _ = ReleaseDC(None, screen_dc);
                        return Err(Error::from_thread());
                    }
                    hbitmap
                }
                Err(e) => {
                    let _ = DeleteDC(mem_dc);
                    let _ = ReleaseDC(None, screen_dc);
                    return Err(e);
                }
            };

            let old_bitmap = SelectObject(mem_dc, dib.into());

            let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 0.0,
                dpiY: 0.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };

            let rt = self.d2d_factory.CreateDCRenderTarget(&rt_props)?;

            self.cached_screen_dc.set(screen_dc);
            self.cached_mem_dc.set(mem_dc);
            self.cached_dib.set(dib);
            self.cached_bits.set(bits);
            self.cached_old_bmp.set(old_bitmap);
            *self.cached_rt.borrow_mut() = Some(rt);
            self.cached_capacity_w.set(alloc_w);
            self.cached_capacity_h.set(alloc_h);
        }

        Ok(())
    }

    /// Creates a DirectWrite text format with primary font family and graceful fallback.
    fn create_text_format(
        &self,
        font_size: f32,
        weight: windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT,
        style: windows::Win32::Graphics::DirectWrite::DWRITE_FONT_STYLE,
    ) -> Result<IDWriteTextFormat> {
        let locale: Vec<u16> = "en-us\0".encode_utf16().collect();
        let format_result = unsafe {
            self.dwrite_factory.CreateTextFormat(
                FONT_FAMILY_PRIMARY,
                None,
                weight,
                style,
                DWRITE_FONT_STRETCH_NORMAL,
                font_size,
                windows::core::PCWSTR(locale.as_ptr()),
            )
        };

        match format_result {
            Ok(fmt) => Ok(fmt),
            Err(_) => unsafe {
                // Fallback to standard Segoe UI if Segoe UI Variable Text is absent
                self.dwrite_factory.CreateTextFormat(
                    FONT_FAMILY_FALLBACK,
                    None,
                    weight,
                    style,
                    DWRITE_FONT_STRETCH_NORMAL,
                    font_size,
                    windows::core::PCWSTR(locale.as_ptr()),
                )
            },
        }
    }

    /// Mathematically generates a hardware-notch path geometry with smooth
    /// upper shoulder concave-corner curvature flowing seamlessly into the top display border
    /// and rounded lower corners.
    pub fn create_notch_geometry(&self, dimensions: &NotchDimensions) -> Result<ID2D1PathGeometry> {
        let pad_x = dimensions.shadow_margin_x;
        let notch_w = dimensions.notch_width();
        let notch_h = dimensions.notch_height();
        let r_top_x = dimensions
            .curvature
            .top_transition_radius
            .max(0.0)
            .min(notch_h / 2.0)
            .min(notch_w / 4.0);
        let r_top_y = dimensions
            .curvature
            .top_transition_height
            .max(0.0)
            .min(notch_h / 2.0)
            .min(notch_w / 4.0);
        let r_bottom = dimensions
            .curvature
            .bottom_radius
            .max(0.0)
            .min(notch_h / 2.0)
            .min(notch_w / 4.0);
        let border_width = dimensions.border_width;
        let half_border = border_width / 2.0;

        let path = unsafe { self.d2d_factory.CreatePathGeometry()? };
        let sink = unsafe { path.Open()? };
        let raw_sink = windows::core::Interface::as_raw(&sink);
        let helper = unsafe { GeometrySinkHelper::from_raw(raw_sink) };

        // Standard cubic Bezier quarter-circle constant (0.5522848)
        let kb = 0.5522848f32;
        let (c1, c2) = (kb, 1.0f32 - kb);

        unsafe {
            let w = pad_x + notch_w - half_border;
            let h = notch_h - half_border;
            let start_x = pad_x + half_border;
            let start_y = 0.0f32; // Anchored at physical screen edge y=0

            // Start at top-left ear attached to screen edge (y = 0)
            helper.begin_figure(D2D_POINT_2F {
                x: start_x,
                y: start_y,
            });

            // 1. Top-left concave shoulder: curves from (start_x, start_y) down to (start_x + r_top_x, start_y + r_top_y)
            if r_top_x > 0.0 && r_top_y > 0.0 {
                helper.add_bezier(&D2D1_BEZIER_SEGMENT {
                    point1: D2D_POINT_2F {
                        x: start_x + c1 * r_top_x,
                        y: start_y,
                    },
                    point2: D2D_POINT_2F {
                        x: start_x + r_top_x,
                        y: start_y + c2 * r_top_y,
                    },
                    point3: D2D_POINT_2F {
                        x: start_x + r_top_x,
                        y: start_y + r_top_y,
                    },
                });
            } else {
                helper.add_line(D2D_POINT_2F {
                    x: start_x + r_top_x,
                    y: start_y + r_top_y,
                });
            }

            // 2. Left vertical wall — from shoulder down to bottom corner start
            helper.add_line(D2D_POINT_2F {
                x: start_x + r_top_x,
                y: h - r_bottom,
            });

            // 3. Bottom-left convex rounded corner
            if r_bottom > 0.0 {
                helper.add_bezier(&D2D1_BEZIER_SEGMENT {
                    point1: D2D_POINT_2F {
                        x: start_x + r_top_x,
                        y: h - r_bottom * (1.0 - kb),
                    },
                    point2: D2D_POINT_2F {
                        x: start_x + r_top_x + r_bottom * (1.0 - kb),
                        y: h,
                    },
                    point3: D2D_POINT_2F {
                        x: start_x + r_top_x + r_bottom,
                        y: h,
                    },
                });
            }

            // 4. Bottom horizontal edge
            helper.add_line(D2D_POINT_2F {
                x: w - r_top_x - r_bottom,
                y: h,
            });

            // 5. Bottom-right convex rounded corner
            if r_bottom > 0.0 {
                helper.add_bezier(&D2D1_BEZIER_SEGMENT {
                    point1: D2D_POINT_2F {
                        x: w - r_top_x - r_bottom + r_bottom * kb,
                        y: h,
                    },
                    point2: D2D_POINT_2F {
                        x: w - r_top_x,
                        y: h - r_bottom * (1.0 - kb),
                    },
                    point3: D2D_POINT_2F {
                        x: w - r_top_x,
                        y: h - r_bottom,
                    },
                });
            }

            // 6. Right vertical wall — up to shoulder endpoint
            helper.add_line(D2D_POINT_2F {
                x: w - r_top_x,
                y: start_y + r_top_y,
            });

            // 7. Top-right concave shoulder: curves from (w - r_top_x, start_y + r_top_y) up to (w, start_y)
            if r_top_x > 0.0 && r_top_y > 0.0 {
                helper.add_bezier(&D2D1_BEZIER_SEGMENT {
                    point1: D2D_POINT_2F {
                        x: w - r_top_x,
                        y: start_y + c2 * r_top_y,
                    },
                    point2: D2D_POINT_2F {
                        x: w - c1 * r_top_x,
                        y: start_y,
                    },
                    point3: D2D_POINT_2F { x: w, y: start_y },
                });
            } else {
                helper.add_line(D2D_POINT_2F { x: w, y: start_y });
            }

            // 8. Top edge: runs all the way left back to left screen edge (y=0 attachment)
            helper.add_line(D2D_POINT_2F {
                x: start_x,
                y: start_y,
            });

            helper.end_figure();
            helper.close()?;
        }

        Ok(path)
    }

    /// Mathematically generates an open notch shadow path along the shoulders, side walls,
    /// and bottom corners/edge, with an optional vertical drop offset `dy`.
    #[allow(dead_code)]
    pub fn create_notch_shadow_path(
        &self,
        dimensions: &NotchDimensions,
        dy: f32,
    ) -> Result<ID2D1PathGeometry> {
        let pad_x = dimensions.shadow_margin_x;
        let notch_w = dimensions.notch_width();
        let notch_h = dimensions.notch_height();
        let r_top_x = dimensions
            .curvature
            .top_transition_radius
            .max(0.0)
            .min(notch_h / 2.0)
            .min(notch_w / 4.0);
        let r_top_y = dimensions
            .curvature
            .top_transition_height
            .max(0.0)
            .min(notch_h / 2.0)
            .min(notch_w / 4.0);
        let r_bottom = dimensions
            .curvature
            .bottom_radius
            .max(0.0)
            .min(notch_h / 2.0)
            .min(notch_w / 4.0);
        let border_width = dimensions.border_width;
        let half_border = border_width / 2.0;

        let path = unsafe { self.d2d_factory.CreatePathGeometry()? };
        let sink = unsafe { path.Open()? };
        let raw_sink = windows::core::Interface::as_raw(&sink);
        let helper = unsafe { GeometrySinkHelper::from_raw(raw_sink) };

        let kb = 0.5522848f32;
        let (c1, c2) = (kb, 1.0f32 - kb);

        unsafe {
            let start_x = pad_x + half_border;
            let start_y = dy;
            let w = pad_x + notch_w - half_border;
            let h = notch_h - half_border + dy;

            // Start at top-left ear attached to screen edge
            helper.begin_figure(D2D_POINT_2F {
                x: start_x,
                y: start_y,
            });

            // 1. Top-left concave shoulder
            if r_top_x > 0.0 && r_top_y > 0.0 {
                helper.add_bezier(&D2D1_BEZIER_SEGMENT {
                    point1: D2D_POINT_2F {
                        x: start_x + c1 * r_top_x,
                        y: start_y,
                    },
                    point2: D2D_POINT_2F {
                        x: start_x + r_top_x,
                        y: start_y + c2 * r_top_y,
                    },
                    point3: D2D_POINT_2F {
                        x: start_x + r_top_x,
                        y: start_y + r_top_y,
                    },
                });
            } else {
                helper.add_line(D2D_POINT_2F {
                    x: start_x + r_top_x,
                    y: start_y + r_top_y,
                });
            }

            // 2. Left vertical wall
            helper.add_line(D2D_POINT_2F {
                x: start_x + r_top_x,
                y: h - r_bottom,
            });

            // 3. Bottom-left convex rounded corner
            if r_bottom > 0.0 {
                helper.add_bezier(&D2D1_BEZIER_SEGMENT {
                    point1: D2D_POINT_2F {
                        x: start_x + r_top_x,
                        y: h - r_bottom * (1.0 - kb),
                    },
                    point2: D2D_POINT_2F {
                        x: start_x + r_top_x + r_bottom * (1.0 - kb),
                        y: h,
                    },
                    point3: D2D_POINT_2F {
                        x: start_x + r_top_x + r_bottom,
                        y: h,
                    },
                });
            }

            // 4. Bottom horizontal edge
            helper.add_line(D2D_POINT_2F {
                x: w - r_top_x - r_bottom,
                y: h,
            });

            // 5. Bottom-right convex rounded corner
            if r_bottom > 0.0 {
                helper.add_bezier(&D2D1_BEZIER_SEGMENT {
                    point1: D2D_POINT_2F {
                        x: w - r_top_x - r_bottom + r_bottom * kb,
                        y: h,
                    },
                    point2: D2D_POINT_2F {
                        x: w - r_top_x,
                        y: h - r_bottom * (1.0 - kb),
                    },
                    point3: D2D_POINT_2F {
                        x: w - r_top_x,
                        y: h - r_bottom,
                    },
                });
            }

            // 6. Right vertical wall
            helper.add_line(D2D_POINT_2F {
                x: w - r_top_x,
                y: start_y + r_top_y,
            });

            // 7. Top-right concave shoulder: curves up to screen edge
            if r_top_x > 0.0 && r_top_y > 0.0 {
                helper.add_bezier(&D2D1_BEZIER_SEGMENT {
                    point1: D2D_POINT_2F {
                        x: w - r_top_x,
                        y: start_y + c2 * r_top_y,
                    },
                    point2: D2D_POINT_2F {
                        x: w - c1 * r_top_x,
                        y: start_y,
                    },
                    point3: D2D_POINT_2F { x: w, y: start_y },
                });
            } else {
                helper.add_line(D2D_POINT_2F { x: w, y: start_y });
            }

            // Open figure: ends at right screen attachment without crossing the top
            helper.end_figure_open();
            helper.close()?;
        }

        Ok(path)
    }
}

/// Applies a soft, dark ambient glow / shadow around the notch silhouette into `bits`.
///
/// This performs an isotropic 3-pass separable box blur (Gaussian approximation)
/// of the notch alpha channel, curving seamlessly around all concave and convex
/// edges of the notch geometry without line-cap artifacts, stepped banding, or vertical displacement.
#[allow(clippy::too_many_arguments)]
pub fn apply_ambient_shadow(
    bits: *mut u8,
    width: usize,
    height: usize,
    stride: usize,
    scale: f32,
    shadow_opacity: f32,
    scratch_a: &mut Vec<u8>,
    scratch_b: &mut Vec<u8>,
) {
    if width == 0 || height == 0 || shadow_opacity <= 0.0 || bits.is_null() {
        return;
    }

    let total_pixels = width * height;
    if scratch_a.len() < total_pixels {
        scratch_a.resize(total_pixels, 0);
    }
    if scratch_b.len() < total_pixels {
        scratch_b.resize(total_pixels, 0);
    }

    // 1. Extract alpha channel from rendered DIB into scratch_a
    unsafe {
        for y in 0..height {
            let row_offset = y * stride * 4;
            let target_offset = y * width;
            for x in 0..width {
                scratch_a[target_offset + x] = *bits.add(row_offset + x * 4 + 3);
            }
        }
    }

    // 2. Perform 3-pass separable box blur (horizontal + vertical)
    // Radius scales with DPI (e.g. r=3 at 100%, r=6 at 200%)
    let radius = ((3.0 * scale).round() as usize).max(1);

    for _ in 0..3 {
        box_blur_horizontal(
            &scratch_a[..total_pixels],
            &mut scratch_b[..total_pixels],
            width,
            height,
            radius,
        );
        box_blur_vertical(
            &scratch_b[..total_pixels],
            &mut scratch_a[..total_pixels],
            width,
            height,
            radius,
        );
    }

    // 3. Composite shadow underneath the notch into bits
    let opacity_factor = (shadow_opacity * 1.25).min(1.0);

    unsafe {
        for y in 0..height {
            let row_offset = y * stride * 4;
            let target_offset = y * width;
            for x in 0..width {
                let pixel_ptr = bits.add(row_offset + x * 4);
                let orig_a = *pixel_ptr.add(3) as u32;

                if orig_a < 255 {
                    let blurred_val = scratch_a[target_offset + x] as f32;
                    let shadow_a = ((blurred_val * opacity_factor).round() as u32).min(255);

                    if shadow_a > 0 {
                        // Composite: Final_A = Orig_A + Shadow_A * (255 - Orig_A) / 255
                        let new_a = orig_a + (shadow_a * (255 - orig_a) + 127) / 255;
                        *pixel_ptr.add(3) = new_a.min(255) as u8;
                    }
                }
            }
        }
    }
}

#[inline]
fn box_blur_horizontal(src: &[u8], dst: &mut [u8], width: usize, height: usize, radius: usize) {
    let window_size = (radius * 2 + 1) as u32;

    for y in 0..height {
        let row_offset = y * width;
        let mut sum: u32 = 0;

        for x_rel in -(radius as isize)..=(radius as isize) {
            let clamped_x = x_rel.clamp(0, (width - 1) as isize) as usize;
            sum += src[row_offset + clamped_x] as u32;
        }
        dst[row_offset] = ((sum + window_size / 2) / window_size) as u8;

        for x in 1..width {
            let add_x = (x + radius).min(width - 1);
            let sub_x = (x as isize - radius as isize - 1).max(0) as usize;
            sum += src[row_offset + add_x] as u32;
            sum -= src[row_offset + sub_x] as u32;
            dst[row_offset + x] = ((sum + window_size / 2) / window_size) as u8;
        }
    }
}

#[inline]
fn box_blur_vertical(src: &[u8], dst: &mut [u8], width: usize, height: usize, radius: usize) {
    let window_size = (radius * 2 + 1) as u32;

    for x in 0..width {
        let mut sum: u32 = 0;

        for y_rel in -(radius as isize)..=(radius as isize) {
            let clamped_y = y_rel.clamp(0, (height - 1) as isize) as usize;
            sum += src[clamped_y * width + x] as u32;
        }
        dst[x] = ((sum + window_size / 2) / window_size) as u8;

        for y in 1..height {
            let add_y = (y + radius).min(height - 1);
            let sub_y = (y as isize - radius as isize - 1).max(0) as usize;
            sum += src[add_y * width + x] as u32;
            sum -= src[sub_y * width + x] as u32;
            dst[y * width + x] = ((sum + window_size / 2) / window_size) as u8;
        }
    }
}

impl Renderer {
    /// Renders the components of the collapsed notch: centered live Windows time
    fn render_collapsed_content(
        &self,
        rt: &ID2D1RenderTarget,
        layout: &CollapsedLayout,
        dimensions: &NotchDimensions,
        formatted_time: &str,
    ) -> Result<()> {
        let clip_rect = layout.content_bounds.to_d2d_rect();
        unsafe {
            rt.PushAxisAlignedClip(&clip_rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }

        let clock_format = self.create_text_format(
            dimensions.font_size_clock,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_FONT_STYLE_NORMAL,
        )?;
        unsafe {
            clock_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            clock_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        }
        let time_utf16: Vec<u16> = formatted_time.encode_utf16().collect();
        let time_rect = layout.clock_bounds.to_d2d_rect();
        let brush = unsafe { rt.CreateSolidColorBrush(&COLOR_TEXT_PRIMARY, None)? };
        unsafe {
            rt.DrawText(
                &time_utf16,
                &clock_format,
                &time_rect,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
            rt.PopAxisAlignedClip();
        }

        Ok(())
    }

    /// Renders the components of the expanded notch: centered live Windows time and date
    fn render_expanded_content(
        &self,
        rt: &ID2D1RenderTarget,
        layout: &ExpandedLayout,
        dimensions: &NotchDimensions,
        formatted_time: &str,
        formatted_date: &str,
    ) -> Result<()> {
        let clip_rect = layout.content_bounds.to_d2d_rect();
        unsafe {
            rt.PushAxisAlignedClip(&clip_rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }

        // 1. Primary live time (approx 28 DIP, Semibold, COLOR_TEXT_PRIMARY)
        let time_format = self.create_text_format(
            dimensions.font_size_expanded_time,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_FONT_STYLE_NORMAL,
        )?;
        unsafe {
            time_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            time_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        }
        let time_utf16: Vec<u16> = formatted_time.encode_utf16().collect();
        let time_rect = layout.time_bounds.to_d2d_rect();
        let primary_brush = unsafe { rt.CreateSolidColorBrush(&COLOR_TEXT_PRIMARY, None)? };
        unsafe {
            rt.DrawText(
                &time_utf16,
                &time_format,
                &time_rect,
                &primary_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }

        // 2. Secondary live date (approx 12 DIP, Regular, COLOR_TEXT_SECONDARY)
        let date_format = self.create_text_format(
            dimensions.font_size_expanded_date,
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_FONT_STYLE_NORMAL,
        )?;
        unsafe {
            date_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            date_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        }
        let date_utf16: Vec<u16> = formatted_date.encode_utf16().collect();
        let date_rect = layout.date_bounds.to_d2d_rect();
        let secondary_brush = unsafe { rt.CreateSolidColorBrush(&COLOR_TEXT_SECONDARY, None)? };
        unsafe {
            rt.DrawText(
                &date_utf16,
                &date_format,
                &date_rect,
                &secondary_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
            rt.PopAxisAlignedClip();
        }

        Ok(())
    }

    /// Renders the notch shape into a 32-bit premultiplied ARGB DIB surface
    /// and uploads it atomically to the Desktop Window Manager (DWM) via UpdateLayeredWindow.
    ///
    /// Reuses cached high-performance GDI/D2D surfaces to guarantee 60+ FPS zero-allocation animation.
    pub fn render(
        &self,
        hwnd: HWND,
        x: i32,
        y: i32,
        dimensions: &NotchDimensions,
        hovered: bool,
        clock: &ClockDateState,
    ) -> Result<()> {
        let width = dimensions.width;
        let height = dimensions.height;
        let border_width = dimensions.border_width;

        if width <= 0 || height <= 0 {
            return Ok(());
        }

        self.ensure_buffer(width, height)?;

        let mem_dc = self.cached_mem_dc.get();
        let screen_dc = self.cached_screen_dc.get();
        let rt_borrow = self.cached_rt.borrow();
        let rt = match rt_borrow.as_ref() {
            Some(target) => target,
            None => return Err(Error::from_thread()),
        };

        unsafe {
            let bind_rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            rt.BindDC(mem_dc, &bind_rect)?;

            rt.BeginDraw();

            // Transparent background (clears all alpha to 0)
            rt.Clear(Some(&COLOR_TRANSPARENT));

            // Solid AMOLED pure black notch body (#000000)
            let fill_brush = rt.CreateSolidColorBrush(&NOTCH_BG_COLOR, None)?;

            // Build mathematical hardware-notch path geometry
            let path = self.create_notch_geometry(dimensions)?;

            // Fill interior with pure black
            rt.FillGeometry(&path, &fill_brush, None);

            // Specular highlight border only if not transparent (preserves deep black on hover)
            let border_color = if hovered {
                COLOR_BORDER_HOVER
            } else {
                NOTCH_BORDER_COLOR
            };
            if border_color.a > 0.0 {
                let border_brush = rt.CreateSolidColorBrush(&border_color, None)?;
                rt.DrawGeometry(&path, &border_brush, border_width, None);
            }

            // Render layout components based on state and height
            let layout = resolve_layout(dimensions);
            match &layout {
                ResolvedLayout::Collapsed { components, .. } => {
                    self.render_collapsed_content(
                        rt,
                        components,
                        dimensions,
                        &clock.formatted_time,
                    )?;
                }
                ResolvedLayout::Expanded { components, .. } => {
                    let min_content_height = (50.0 * dimensions.scale).round() as i32;
                    if dimensions.height >= min_content_height {
                        self.render_expanded_content(
                            rt,
                            components,
                            dimensions,
                            &clock.formatted_time,
                            &clock.formatted_date,
                        )?;
                    } else {
                        let collapsed_layout = resolve_collapsed_layout(dimensions);
                        self.render_collapsed_content(
                            rt,
                            &collapsed_layout,
                            dimensions,
                            &clock.formatted_time,
                        )?;
                    }
                }
            }

            rt.EndDraw(None, None)?;

            // Soft dark ambient shadow for expanded notch
            if dimensions.shadow_opacity > 0.0 {
                let bits = self.cached_bits.get();
                if !bits.is_null() {
                    let mut scratch_a = self.scratch_a.borrow_mut();
                    let mut scratch_b = self.scratch_b.borrow_mut();
                    apply_ambient_shadow(
                        bits as *mut u8,
                        width as usize,
                        height as usize,
                        self.cached_capacity_w.get() as usize,
                        dimensions.scale,
                        dimensions.shadow_opacity,
                        &mut scratch_a,
                        &mut scratch_b,
                    );
                }
            }

            // Upload to DWM atomically via UpdateLayeredWindow
            let pt_dst = POINT { x, y };
            let size = SIZE {
                cx: width,
                cy: height,
            };
            let pt_src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };

            UpdateLayeredWindow(
                hwnd,
                Some(screen_dc),
                Some(&pt_dst),
                Some(&size),
                Some(mem_dc),
                Some(&pt_src),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.cleanup_cached_resources();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NotchState;

    #[test]
    fn test_renderer_initialization_and_drawing() {
        let renderer = Renderer::new().expect("Failed to initialize Direct2D renderer");
        let dims = NotchDimensions::from_dpi(96);
        let width = dims.width;
        let height = dims.height;
        let border_width = dims.border_width;

        unsafe {
            let screen_dc = GetDC(None);
            assert!(!screen_dc.is_invalid());
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            assert!(!mem_dc.is_invalid());

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };

            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let dib = CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
                .expect("Failed to create DIB section");
            let old_bitmap = SelectObject(mem_dc, dib.into());

            let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 0.0,
                dpiY: 0.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };

            let rt = renderer
                .d2d_factory
                .CreateDCRenderTarget(&rt_props)
                .expect("Failed to create DC render target");
            let bind_rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            rt.BindDC(mem_dc, &bind_rect).expect("Failed to bind DC");

            rt.BeginDraw();
            rt.Clear(Some(&COLOR_TRANSPARENT));

            let fill_brush = rt
                .CreateSolidColorBrush(&NOTCH_BG_COLOR, None)
                .expect("Failed to create fill brush");
            let border_brush = rt
                .CreateSolidColorBrush(&NOTCH_BORDER_COLOR, None)
                .expect("Failed to create border brush");

            let path = renderer
                .create_notch_geometry(&dims)
                .expect("Failed to create notch path geometry");

            rt.FillGeometry(&path, &fill_brush, None);
            rt.DrawGeometry(&path, &border_brush, border_width, None);

            let layout = resolve_layout(&dims);
            let clock_state =
                ClockDateState::from_pure_components(2026, 10, 5, 1, 17, 42, 0, 0, false);
            if let ResolvedLayout::Collapsed { components, .. } = &layout {
                renderer
                    .render_collapsed_content(&rt, components, &dims, &clock_state.formatted_time)
                    .expect("render collapsed content failed");
            } else {
                panic!("Expected ResolvedLayout::Collapsed");
            }

            rt.EndDraw(None, None).expect("EndDraw failed");

            // Verify rendered pixels
            assert!(!bits.is_null());
            let pixel_slice =
                std::slice::from_raw_parts(bits as *const u8, (width * height * 4) as usize);

            // Left area (x=50, y=16) inside the notch surface to the left of the centered clock must be solid black (alpha=255, r=0, g=0, b=0)
            let left_safe_idx = ((16 * width + 50) * 4) as usize;
            assert_eq!(
                pixel_slice[left_safe_idx + 3],
                255,
                "Inside notch pixel should have alpha = 255"
            );
            assert_eq!(pixel_slice[left_safe_idx], 0, "Blue should be 0");
            assert_eq!(pixel_slice[left_safe_idx + 1], 0, "Green should be 0");
            assert_eq!(pixel_slice[left_safe_idx + 2], 0, "Red should be 0");

            // Right area (x=170, y=16) inside the notch surface to the right of the centered clock must be solid black (alpha=255, r=0, g=0, b=0)
            let right_safe_idx = ((16 * width + 170) * 4) as usize;
            assert_eq!(
                pixel_slice[right_safe_idx + 3],
                255,
                "Right inside notch pixel should have alpha = 255"
            );
            assert_eq!(pixel_slice[right_safe_idx], 0, "Blue should be 0");
            assert_eq!(pixel_slice[right_safe_idx + 1], 0, "Green should be 0");
            assert_eq!(pixel_slice[right_safe_idx + 2], 0, "Red should be 0");

            // Check that old emblem area (x in 20..40, y in 8..24) contains ZERO non-black pixels (emblem completely removed)
            let mut old_emblem_non_black_count = 0;
            for y in 8..24 {
                for x in 20..40 {
                    let idx = ((y * width + x) * 4) as usize;
                    let r = pixel_slice[idx + 2];
                    let g = pixel_slice[idx + 1];
                    let b = pixel_slice[idx];
                    if r > 0 || g > 0 || b > 0 {
                        old_emblem_non_black_count += 1;
                    }
                }
            }
            assert_eq!(
                old_emblem_non_black_count, 0,
                "Old emblem area (x in 20..40) must be pure black with emblem removed (found {old_emblem_non_black_count} non-black pixels)"
            );

            // Check that centered live time is actually rendered in the center region (x in 70..150, y in 8..24)
            let mut center_time_pixel_count = 0;
            for y in 8..24 {
                for x in 70..150 {
                    let idx = ((y * width + x) * 4) as usize;
                    let r = pixel_slice[idx + 2];
                    let g = pixel_slice[idx + 1];
                    let b = pixel_slice[idx];
                    if r > 20 || g > 20 || b > 20 {
                        center_time_pixel_count += 1;
                    }
                }
            }
            assert!(
                center_time_pixel_count > 25,
                "Collapsed surface should contain rendered centered time text (found {center_time_pixel_count} pixels)"
            );

            // Top screen attachment pixel (x=110, y=0) must be attached and opaque (alpha=255)
            let top_center_idx = (110 * 4) as usize;
            assert_eq!(
                pixel_slice[top_center_idx + 3],
                255,
                "Top attachment center should be opaque"
            );

            // Corner pixel (x=1, y=1) outside shoulder curve must be transparent (alpha=0)
            let corner_idx = ((width + 1) * 4) as usize;
            assert_eq!(
                pixel_slice[corner_idx + 3],
                0,
                "Top-left corner pixel outside shoulder radius should be transparent"
            );

            // Outside left wall (x=2, y=16) must be transparent (alpha=0)
            let left_outside_idx = ((16 * width + 2) * 4) as usize;
            assert_eq!(
                pixel_slice[left_outside_idx + 3],
                0,
                "Pixel to the left of the notch body should be transparent"
            );

            // Save rendered notch bitmap to artifact directory for visual verification
            let bmp_path = r"C:\Users\ujval\.gemini\antigravity-ide\brain\402578f9-d9e5-44d8-9b61-4772077cf14b\notch_hardware_notch.bmp";
            let mut file_bytes = Vec::with_capacity(54 + pixel_slice.len());
            file_bytes.extend_from_slice(b"BM");
            let file_size = (54 + pixel_slice.len()) as u32;
            file_bytes.extend_from_slice(&file_size.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&54u32.to_le_bytes());
            file_bytes.extend_from_slice(&40u32.to_le_bytes());
            file_bytes.extend_from_slice(&width.to_le_bytes());
            file_bytes.extend_from_slice(&(-height).to_le_bytes());
            file_bytes.extend_from_slice(&1u16.to_le_bytes());
            file_bytes.extend_from_slice(&32u16.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&(pixel_slice.len() as u32).to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(pixel_slice);
            let _ = std::fs::write(bmp_path, file_bytes);

            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteObject(dib.into());
            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);
        }
    }

    #[test]
    fn test_renderer_expanded_state_drawing() {
        let renderer = Renderer::new().expect("Failed to initialize Direct2D renderer");
        let dims = NotchDimensions::from_state_and_dpi(NotchState::Expanded, 96);
        let width = dims.width;
        let height = dims.height;
        let border_width = dims.border_width;

        assert_eq!(width, 600);
        assert_eq!(height, 120);

        unsafe {
            let screen_dc = GetDC(None);
            assert!(!screen_dc.is_invalid());
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            assert!(!mem_dc.is_invalid());

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };

            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let dib = CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
                .expect("Failed to create DIB section");
            let old_bitmap = SelectObject(mem_dc, dib.into());

            let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 0.0,
                dpiY: 0.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };

            let rt = renderer
                .d2d_factory
                .CreateDCRenderTarget(&rt_props)
                .expect("Failed to create DC render target");
            let bind_rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            rt.BindDC(mem_dc, &bind_rect).expect("Failed to bind DC");

            rt.BeginDraw();
            rt.Clear(Some(&COLOR_TRANSPARENT));

            let fill_brush = rt
                .CreateSolidColorBrush(&NOTCH_BG_COLOR, None)
                .expect("Failed to create fill brush");
            let border_brush = rt
                .CreateSolidColorBrush(&NOTCH_BORDER_COLOR, None)
                .expect("Failed to create border brush");

            let path = renderer
                .create_notch_geometry(&dims)
                .expect("Failed to create notch path geometry");

            rt.FillGeometry(&path, &fill_brush, None);
            rt.DrawGeometry(&path, &border_brush, border_width, None);

            let clock_state =
                ClockDateState::from_pure_components(2026, 10, 6, 2, 0, 47, 0, 0, false);
            let layout = resolve_layout(&dims);
            if let ResolvedLayout::Expanded { components, .. } = &layout {
                renderer
                    .render_expanded_content(
                        &rt,
                        components,
                        &dims,
                        &clock_state.formatted_time,
                        &clock_state.formatted_date,
                    )
                    .expect("render expanded content failed");
            } else {
                panic!("Expected ResolvedLayout::Expanded");
            }

            rt.EndDraw(None, None).expect("EndDraw failed");

            // Apply soft dark ambient shadow
            if dims.shadow_opacity > 0.0 {
                let mut scratch_a = renderer.scratch_a.borrow_mut();
                let mut scratch_b = renderer.scratch_b.borrow_mut();
                apply_ambient_shadow(
                    bits as *mut u8,
                    width as usize,
                    height as usize,
                    width as usize,
                    dims.scale,
                    dims.shadow_opacity,
                    &mut scratch_a,
                    &mut scratch_b,
                );
            }

            // Verify rendered pixels
            assert!(!bits.is_null());
            let pixel_slice =
                std::slice::from_raw_parts(bits as *const u8, (width * height * 4) as usize);

            // Center pixel (x=300, y=55) inside the expanded notch must be opaque (alpha=255) and pure AMOLED black (RGB=0)
            let center_idx = ((55 * width + 300) * 4) as usize;
            assert_eq!(
                pixel_slice[center_idx + 3],
                255,
                "Center pixel should have alpha = 255"
            );

            // Top screen attachment pixel (x=300, y=0) must be attached and opaque (alpha=255)
            let top_center_idx = (300 * 4) as usize;
            assert_eq!(
                pixel_slice[top_center_idx + 3],
                255,
                "Top attachment center should be opaque"
            );

            // Top-left shoulder ear (x=1, y=1) outside shoulder radius must be transparent (alpha=0)
            let corner_idx = ((width + 1) * 4) as usize;
            assert_eq!(
                pixel_slice[corner_idx + 3],
                0,
                "Top-left corner pixel outside shoulder radius should be transparent"
            );

            // Outside shadow perimeter (x=1, y=55) has near-zero or zero alpha
            let far_left_idx = ((55 * width + 1) * 4) as usize;
            assert!(
                pixel_slice[far_left_idx + 3] < 15,
                "Pixel at window boundary should be transparent/near-zero shadow"
            );

            // Ambient shadow in margin outside notch wall (x=20, y=55) has soft dark alpha
            let shadow_sample_idx = ((55 * width + 20) * 4) as usize;
            let shadow_a = pixel_slice[shadow_sample_idx + 3];
            assert!(
                shadow_a > 0 && shadow_a < 120,
                "Ambient shadow should exist outside wall with soft alpha (got {shadow_a})"
            );

            // Verify that live time and date are rendered in the center region
            let mut text_pixel_count = 0;
            for y in 30..90 {
                for x in 150..(width - 150) {
                    let idx = ((y * width + x) * 4) as usize;
                    let b = pixel_slice[idx];
                    let g = pixel_slice[idx + 1];
                    let r = pixel_slice[idx + 2];
                    let a = pixel_slice[idx + 3];
                    if a == 255 && (r > 0 || g > 0 || b > 0) {
                        text_pixel_count += 1;
                    }
                }
            }
            assert!(
                text_pixel_count > 50,
                "Expanded surface should display rendered time and date text (found {text_pixel_count} text pixels)"
            );

            // Verify left safe-area inside notch body remains pure AMOLED black background
            let mut left_text_pixel_count = 0;
            for y in 35..85 {
                for x in 50..100 {
                    let idx = ((y * width + x) * 4) as usize;
                    let b = pixel_slice[idx];
                    let g = pixel_slice[idx + 1];
                    let r = pixel_slice[idx + 2];
                    if r > 0 || g > 0 || b > 0 {
                        left_text_pixel_count += 1;
                    }
                }
            }
            assert_eq!(
                left_text_pixel_count, 0,
                "Left padding area should be pure black (no stray text pixels)"
            );

            // Save rendered expanded notch bitmap to artifact directory for visual verification
            let bmp_path = r"C:\Users\ujval\.gemini\antigravity-ide\brain\402578f9-d9e5-44d8-9b61-4772077cf14b\notch_expanded.bmp";
            let mut file_bytes = Vec::with_capacity(54 + pixel_slice.len());
            file_bytes.extend_from_slice(b"BM");
            let file_size = (54 + pixel_slice.len()) as u32;
            file_bytes.extend_from_slice(&file_size.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&54u32.to_le_bytes());
            file_bytes.extend_from_slice(&40u32.to_le_bytes());
            file_bytes.extend_from_slice(&width.to_le_bytes());
            file_bytes.extend_from_slice(&(-height).to_le_bytes());
            file_bytes.extend_from_slice(&1u16.to_le_bytes());
            file_bytes.extend_from_slice(&32u16.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&(pixel_slice.len() as u32).to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(pixel_slice);
            let _ = std::fs::write(bmp_path, file_bytes);

            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteObject(dib.into());
            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);
        }
    }

    #[test]
    fn test_renderer_zero_and_negative_dimensions_safety() {
        let renderer = Renderer::new().expect("Failed to initialize Direct2D renderer");
        let hwnd = HWND::default();
        let clock = ClockDateState::default();
        let mut dims_zero = NotchDimensions::from_dpi(96);
        dims_zero.width = 0;
        dims_zero.height = 0;
        // Zero dimensions should safely return Ok(()) without attempting allocation or panic
        assert!(
            renderer
                .render(hwnd, 0, 0, &dims_zero, false, &clock)
                .is_ok()
        );

        let mut dims_negative = NotchDimensions::from_dpi(96);
        dims_negative.width = -50;
        dims_negative.height = -20;
        // Negative dimensions should also safely return Ok(())
        assert!(
            renderer
                .render(hwnd, 0, 0, &dims_negative, false, &clock)
                .is_ok()
        );
    }

    #[test]
    fn test_hovered_rendering_visual_and_subtle_highlight() {
        let renderer = Renderer::new().expect("Failed to initialize Direct2D renderer");
        let dims = NotchDimensions::from_state_and_dpi(NotchState::Collapsed, 96);
        let width = dims.width;
        let height = dims.height;

        unsafe {
            let screen_dc = GetDC(None);
            assert!(!screen_dc.is_invalid());
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            assert!(!mem_dc.is_invalid());

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };

            // 1. Render Hovered Collapsed Notch
            let mut bits_hover: *mut std::ffi::c_void = std::ptr::null_mut();
            let dib_hover =
                CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits_hover, None, 0)
                    .expect("Failed to create DIB section for hover test");
            let old_bmp = SelectObject(mem_dc, dib_hover.into());

            let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };

            let rt = renderer
                .d2d_factory
                .CreateDCRenderTarget(&rt_props)
                .expect("Failed to create DC render target");

            let rc = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            rt.BindDC(mem_dc, &rc).expect("BindDC failed");

            rt.BeginDraw();
            rt.Clear(Some(&COLOR_TRANSPARENT));

            let fill_brush = rt
                .CreateSolidColorBrush(&NOTCH_BG_COLOR, None)
                .expect("Failed to create fill brush");
            let hover_border_brush = rt
                .CreateSolidColorBrush(&COLOR_BORDER_HOVER, None)
                .expect("Failed to create hover border brush");

            let path = renderer
                .create_notch_geometry(&dims)
                .expect("Failed to create notch path geometry");

            rt.FillGeometry(&path, &fill_brush, None);
            rt.DrawGeometry(&path, &hover_border_brush, dims.border_width, None);

            let layout = resolve_layout(&dims);
            if let ResolvedLayout::Collapsed { components, .. } = &layout {
                renderer
                    .render_collapsed_content(&rt, components, &dims, "12:00 PM")
                    .expect("render collapsed content failed");
            }

            rt.EndDraw(None, None).expect("EndDraw failed");

            let pixel_slice =
                std::slice::from_raw_parts(bits_hover as *const u8, (width * height * 4) as usize);

            // Center pixel must remain pure AMOLED #000000
            let center_idx = ((16 * width + 110) * 4) as usize;
            assert_eq!(pixel_slice[center_idx + 3], 255, "Alpha must be 255");
            assert_eq!(pixel_slice[center_idx], 0, "Blue must be 0");
            assert_eq!(pixel_slice[center_idx + 1], 0, "Green must be 0");
            assert_eq!(pixel_slice[center_idx + 2], 0, "Red must be 0");

            // Corner pixel outside shoulder curve must remain transparent
            let corner_idx = ((width + 1) * 4) as usize;
            assert_eq!(
                pixel_slice[corner_idx + 3],
                0,
                "Outside must be transparent"
            );

            // Save rendered hover bitmap
            let bmp_path = r"C:\Users\ujval\.gemini\antigravity-ide\brain\402578f9-d9e5-44d8-9b61-4772077cf14b\notch_hovered_collapsed.bmp";
            let mut file_bytes = Vec::with_capacity(54 + pixel_slice.len());
            file_bytes.extend_from_slice(b"BM");
            let file_size = (54 + pixel_slice.len()) as u32;
            file_bytes.extend_from_slice(&file_size.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&54u32.to_le_bytes());
            file_bytes.extend_from_slice(&40u32.to_le_bytes());
            file_bytes.extend_from_slice(&width.to_le_bytes());
            file_bytes.extend_from_slice(&(-height).to_le_bytes());
            file_bytes.extend_from_slice(&1u16.to_le_bytes());
            file_bytes.extend_from_slice(&32u16.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&(pixel_slice.len() as u32).to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(pixel_slice);
            let _ = std::fs::write(bmp_path, file_bytes);

            SelectObject(mem_dc, old_bmp);
            let _ = DeleteObject(dib_hover.into());

            // 2. Render Hovered Expanded Notch
            let dims_exp = NotchDimensions::from_state_and_dpi(NotchState::Expanded, 96);
            let w_exp = dims_exp.width;
            let h_exp = dims_exp.height;
            let bmi_exp = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w_exp,
                    biHeight: -h_exp,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };

            let mut bits_exp: *mut std::ffi::c_void = std::ptr::null_mut();
            let dib_exp = CreateDIBSection(
                Some(mem_dc),
                &bmi_exp,
                DIB_RGB_COLORS,
                &mut bits_exp,
                None,
                0,
            )
            .expect("Failed to create DIB section for expanded hover test");
            let old_bmp_exp = SelectObject(mem_dc, dib_exp.into());

            let rt_exp = renderer
                .d2d_factory
                .CreateDCRenderTarget(&rt_props)
                .expect("Failed to create DC render target for expanded hover");
            let rc_exp = RECT {
                left: 0,
                top: 0,
                right: w_exp,
                bottom: h_exp,
            };
            rt_exp.BindDC(mem_dc, &rc_exp).expect("BindDC failed");
            rt_exp.BeginDraw();
            rt_exp.Clear(Some(&COLOR_TRANSPARENT));

            let fill_brush_exp = rt_exp
                .CreateSolidColorBrush(&NOTCH_BG_COLOR, None)
                .expect("Failed to create fill brush");
            let hover_border_brush_exp = rt_exp
                .CreateSolidColorBrush(&COLOR_BORDER_HOVER, None)
                .expect("Failed to create hover border brush");

            let path_exp = renderer
                .create_notch_geometry(&dims_exp)
                .expect("Failed to create expanded notch path");
            rt_exp.FillGeometry(&path_exp, &fill_brush_exp, None);
            rt_exp.DrawGeometry(
                &path_exp,
                &hover_border_brush_exp,
                dims_exp.border_width,
                None,
            );

            let layout_exp = resolve_layout(&dims_exp);
            if let ResolvedLayout::Expanded { components, .. } = &layout_exp {
                renderer
                    .render_expanded_content(
                        &rt_exp,
                        components,
                        &dims_exp,
                        "12:00 PM",
                        "Monday, October 6",
                    )
                    .expect("render expanded content failed");
            }

            rt_exp.EndDraw(None, None).expect("EndDraw failed");

            let pixel_slice_exp =
                std::slice::from_raw_parts(bits_exp as *const u8, (w_exp * h_exp * 4) as usize);

            // Save rendered expanded hover bitmap
            let bmp_path_exp = r"C:\Users\ujval\.gemini\antigravity-ide\brain\402578f9-d9e5-44d8-9b61-4772077cf14b\notch_hovered_expanded.bmp";
            let mut file_bytes_exp = Vec::with_capacity(54 + pixel_slice_exp.len());
            file_bytes_exp.extend_from_slice(b"BM");
            let file_size_exp = (54 + pixel_slice_exp.len()) as u32;
            file_bytes_exp.extend_from_slice(&file_size_exp.to_le_bytes());
            file_bytes_exp.extend_from_slice(&0u32.to_le_bytes());
            file_bytes_exp.extend_from_slice(&54u32.to_le_bytes());
            file_bytes_exp.extend_from_slice(&40u32.to_le_bytes());
            file_bytes_exp.extend_from_slice(&w_exp.to_le_bytes());
            file_bytes_exp.extend_from_slice(&(-h_exp).to_le_bytes());
            file_bytes_exp.extend_from_slice(&1u16.to_le_bytes());
            file_bytes_exp.extend_from_slice(&32u16.to_le_bytes());
            file_bytes_exp.extend_from_slice(&0u32.to_le_bytes());
            file_bytes_exp.extend_from_slice(&(pixel_slice_exp.len() as u32).to_le_bytes());
            file_bytes_exp.extend_from_slice(&0u32.to_le_bytes());
            file_bytes_exp.extend_from_slice(&0u32.to_le_bytes());
            file_bytes_exp.extend_from_slice(&0u32.to_le_bytes());
            file_bytes_exp.extend_from_slice(&0u32.to_le_bytes());
            file_bytes_exp.extend_from_slice(pixel_slice_exp);
            let _ = std::fs::write(bmp_path_exp, file_bytes_exp);

            SelectObject(mem_dc, old_bmp_exp);
            let _ = DeleteObject(dib_exp.into());
            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);
        }
    }

    #[test]
    fn test_animation_intermediate_content_clipping() {
        let renderer = Renderer::new().expect("Failed to initialize Direct2D renderer");

        // 1. Expanding animation frame (progress = 0.65)
        let mut anim_exp =
            crate::config::AnimationState::new(NotchState::Collapsed, NotchState::Expanded);
        anim_exp.set_progress(0.65);
        let dims_exp = anim_exp.current_dimensions(96);
        let w_exp = dims_exp.width;
        let h_exp = dims_exp.height;

        unsafe {
            let screen_dc = GetDC(None);
            assert!(!screen_dc.is_invalid());
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            assert!(!mem_dc.is_invalid());

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w_exp,
                    biHeight: -h_exp,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };

            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let dib = CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
                .expect("Failed to create DIB section");
            let old_bmp = SelectObject(mem_dc, dib.into());

            let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };

            let rt = renderer
                .d2d_factory
                .CreateDCRenderTarget(&rt_props)
                .expect("Failed to create DC render target");

            let rc = RECT {
                left: 0,
                top: 0,
                right: w_exp,
                bottom: h_exp,
            };
            rt.BindDC(mem_dc, &rc).expect("BindDC failed");
            rt.BeginDraw();
            rt.Clear(Some(&COLOR_TRANSPARENT));

            let fill_brush = rt
                .CreateSolidColorBrush(&NOTCH_BG_COLOR, None)
                .expect("fill brush");
            let border_brush = rt
                .CreateSolidColorBrush(&NOTCH_BORDER_COLOR, None)
                .expect("border brush");
            let path = renderer.create_notch_geometry(&dims_exp).expect("geometry");
            rt.FillGeometry(&path, &fill_brush, None);
            rt.DrawGeometry(&path, &border_brush, dims_exp.border_width, None);

            let layout = resolve_layout(&dims_exp);
            match &layout {
                ResolvedLayout::Expanded { components, .. } => {
                    let min_h = (50.0 * dims_exp.scale).round() as i32;
                    if dims_exp.height >= min_h {
                        renderer
                            .render_expanded_content(
                                &rt,
                                components,
                                &dims_exp,
                                "12:00 PM",
                                "Monday, October 6",
                            )
                            .expect("render expanded content");
                    }
                }
                ResolvedLayout::Collapsed { components, .. } => {
                    renderer
                        .render_collapsed_content(&rt, components, &dims_exp, "12:00 PM")
                        .expect("render collapsed content");
                }
            }

            rt.EndDraw(None, None).expect("EndDraw failed");

            let pixel_slice =
                std::slice::from_raw_parts(bits as *const u8, (w_exp * h_exp * 4) as usize);

            // Save expanding animation frame bitmap
            let bmp_path = r"C:\Users\ujval\.gemini\antigravity-ide\brain\402578f9-d9e5-44d8-9b61-4772077cf14b\notch_animating_expanding.bmp";
            let mut file_bytes = Vec::with_capacity(54 + pixel_slice.len());
            file_bytes.extend_from_slice(b"BM");
            let file_size = (54 + pixel_slice.len()) as u32;
            file_bytes.extend_from_slice(&file_size.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&54u32.to_le_bytes());
            file_bytes.extend_from_slice(&40u32.to_le_bytes());
            file_bytes.extend_from_slice(&w_exp.to_le_bytes());
            file_bytes.extend_from_slice(&(-h_exp).to_le_bytes());
            file_bytes.extend_from_slice(&1u16.to_le_bytes());
            file_bytes.extend_from_slice(&32u16.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&(pixel_slice.len() as u32).to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(&0u32.to_le_bytes());
            file_bytes.extend_from_slice(pixel_slice);
            let _ = std::fs::write(bmp_path, file_bytes);

            SelectObject(mem_dc, old_bmp);
            let _ = DeleteObject(dib.into());

            // 2. Collapsing animation frame (progress = 0.60)
            let mut anim_col =
                crate::config::AnimationState::new(NotchState::Expanded, NotchState::Collapsed);
            anim_col.set_progress(0.60);
            let dims_col = anim_col.current_dimensions(96);
            let w_col = dims_col.width;
            let h_col = dims_col.height;

            let bmi_col = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w_col,
                    biHeight: -h_col,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };

            let mut bits_col: *mut std::ffi::c_void = std::ptr::null_mut();
            let dib_col = CreateDIBSection(
                Some(mem_dc),
                &bmi_col,
                DIB_RGB_COLORS,
                &mut bits_col,
                None,
                0,
            )
            .expect("create DIB");
            let old_bmp_col = SelectObject(mem_dc, dib_col.into());

            let rt_col = renderer
                .d2d_factory
                .CreateDCRenderTarget(&rt_props)
                .expect("rt_col");
            let rc_col = RECT {
                left: 0,
                top: 0,
                right: w_col,
                bottom: h_col,
            };
            rt_col.BindDC(mem_dc, &rc_col).expect("BindDC");
            rt_col.BeginDraw();
            rt_col.Clear(Some(&COLOR_TRANSPARENT));

            let fill_col = rt_col
                .CreateSolidColorBrush(&NOTCH_BG_COLOR, None)
                .expect("fill");
            let border_col = rt_col
                .CreateSolidColorBrush(&NOTCH_BORDER_COLOR, None)
                .expect("border");
            let path_col = renderer.create_notch_geometry(&dims_col).expect("path");
            rt_col.FillGeometry(&path_col, &fill_col, None);
            rt_col.DrawGeometry(&path_col, &border_col, dims_col.border_width, None);

            let layout_col = resolve_layout(&dims_col);
            match &layout_col {
                ResolvedLayout::Expanded { components, .. } => {
                    let min_h = (50.0 * dims_col.scale).round() as i32;
                    if dims_col.height >= min_h {
                        renderer
                            .render_expanded_content(
                                &rt_col,
                                components,
                                &dims_col,
                                "12:00 PM",
                                "Monday, October 6",
                            )
                            .expect("render expanded");
                    } else {
                        let collapsed_layout = resolve_collapsed_layout(&dims_col);
                        renderer
                            .render_collapsed_content(
                                &rt_col,
                                &collapsed_layout,
                                &dims_col,
                                "12:00 PM",
                            )
                            .expect("render collapsed");
                    }
                }
                ResolvedLayout::Collapsed { components, .. } => {
                    renderer
                        .render_collapsed_content(&rt_col, components, &dims_col, "12:00 PM")
                        .expect("render collapsed");
                }
            }

            rt_col.EndDraw(None, None).expect("EndDraw");

            let pixel_slice_col =
                std::slice::from_raw_parts(bits_col as *const u8, (w_col * h_col * 4) as usize);

            // Save collapsing animation frame bitmap
            let bmp_path_col = r"C:\Users\ujval\.gemini\antigravity-ide\brain\402578f9-d9e5-44d8-9b61-4772077cf14b\notch_animating_collapsing.bmp";
            let mut file_bytes_col = Vec::with_capacity(54 + pixel_slice_col.len());
            file_bytes_col.extend_from_slice(b"BM");
            let file_size_col = (54 + pixel_slice_col.len()) as u32;
            file_bytes_col.extend_from_slice(&file_size_col.to_le_bytes());
            file_bytes_col.extend_from_slice(&0u32.to_le_bytes());
            file_bytes_col.extend_from_slice(&54u32.to_le_bytes());
            file_bytes_col.extend_from_slice(&40u32.to_le_bytes());
            file_bytes_col.extend_from_slice(&w_col.to_le_bytes());
            file_bytes_col.extend_from_slice(&(-h_col).to_le_bytes());
            file_bytes_col.extend_from_slice(&1u16.to_le_bytes());
            file_bytes_col.extend_from_slice(&32u16.to_le_bytes());
            file_bytes_col.extend_from_slice(&0u32.to_le_bytes());
            file_bytes_col.extend_from_slice(&(pixel_slice_col.len() as u32).to_le_bytes());
            file_bytes_col.extend_from_slice(&0u32.to_le_bytes());
            file_bytes_col.extend_from_slice(&0u32.to_le_bytes());
            file_bytes_col.extend_from_slice(&0u32.to_le_bytes());
            file_bytes_col.extend_from_slice(&0u32.to_le_bytes());
            file_bytes_col.extend_from_slice(pixel_slice_col);
            let _ = std::fs::write(bmp_path_col, file_bytes_col);

            SelectObject(mem_dc, old_bmp_col);
            let _ = DeleteObject(dib_col.into());
            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);
        }
    }

    #[test]
    fn test_renderer_clock_12h_and_24h_display() {
        let renderer = Renderer::new().expect("Failed to initialize Direct2D renderer");
        let dims = NotchDimensions::from_state_and_dpi(NotchState::Collapsed, 96);
        let width = dims.width;
        let height = dims.height;

        unsafe {
            let screen_dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(Some(screen_dc));

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };

            let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };

            let rt = renderer
                .d2d_factory
                .CreateDCRenderTarget(&rt_props)
                .expect("Failed to create DC render target");

            let rc = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };

            // Test 1: 12-hour format "5:42 PM"
            {
                let mut bits_12h: *mut std::ffi::c_void = std::ptr::null_mut();
                let dib_12h =
                    CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits_12h, None, 0)
                        .expect("DIB 12h");
                let old_bmp = SelectObject(mem_dc, dib_12h.into());
                rt.BindDC(mem_dc, &rc).expect("BindDC 12h");

                rt.BeginDraw();
                rt.Clear(Some(&COLOR_TRANSPARENT));
                let fill_brush = rt
                    .CreateSolidColorBrush(&NOTCH_BG_COLOR, None)
                    .expect("fill");
                let path = renderer.create_notch_geometry(&dims).expect("geometry");
                rt.FillGeometry(&path, &fill_brush, None);

                let layout = resolve_layout(&dims);
                if let ResolvedLayout::Collapsed { components, .. } = &layout {
                    renderer
                        .render_collapsed_content(&rt, components, &dims, "5:42 PM")
                        .expect("render 12h");
                }
                rt.EndDraw(None, None).expect("EndDraw 12h");

                let pixel_slice = std::slice::from_raw_parts(
                    bits_12h as *const u8,
                    (width * height * 4) as usize,
                );

                // Center time region has rendered text pixels
                let mut text_pixel_count = 0;
                for y in 8..24 {
                    for x in 70..150 {
                        let idx = ((y * width + x) * 4) as usize;
                        let r = pixel_slice[idx + 2];
                        if r > 20 {
                            text_pixel_count += 1;
                        }
                    }
                }
                assert!(text_pixel_count > 20, "12h text must be rendered");

                SelectObject(mem_dc, old_bmp);
                let _ = DeleteObject(dib_12h.into());
            }

            // Test 2: 24-hour format "17:42"
            {
                let mut bits_24h: *mut std::ffi::c_void = std::ptr::null_mut();
                let dib_24h =
                    CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits_24h, None, 0)
                        .expect("DIB 24h");
                let old_bmp = SelectObject(mem_dc, dib_24h.into());
                rt.BindDC(mem_dc, &rc).expect("BindDC 24h");

                rt.BeginDraw();
                rt.Clear(Some(&COLOR_TRANSPARENT));
                let fill_brush = rt
                    .CreateSolidColorBrush(&NOTCH_BG_COLOR, None)
                    .expect("fill");
                let path = renderer.create_notch_geometry(&dims).expect("geometry");
                rt.FillGeometry(&path, &fill_brush, None);

                let layout = resolve_layout(&dims);
                if let ResolvedLayout::Collapsed { components, .. } = &layout {
                    renderer
                        .render_collapsed_content(&rt, components, &dims, "17:42")
                        .expect("render 24h");
                }
                rt.EndDraw(None, None).expect("EndDraw 24h");

                let pixel_slice = std::slice::from_raw_parts(
                    bits_24h as *const u8,
                    (width * height * 4) as usize,
                );

                // Center time region has rendered text pixels
                let mut text_pixel_count = 0;
                for y in 8..24 {
                    for x in 70..150 {
                        let idx = ((y * width + x) * 4) as usize;
                        let r = pixel_slice[idx + 2];
                        if r > 20 {
                            text_pixel_count += 1;
                        }
                    }
                }
                assert!(text_pixel_count > 20, "24h text must be rendered");

                SelectObject(mem_dc, old_bmp);
                let _ = DeleteObject(dib_24h.into());
            }

            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);
        }
    }

    #[test]
    fn test_expanded_renderer_12h_24h_and_date_display() {
        let renderer = Renderer::new().expect("Failed to initialize Direct2D renderer");
        let dims = NotchDimensions::from_state_and_dpi(NotchState::Expanded, 96);
        let width = dims.width;
        let height = dims.height;

        unsafe {
            let screen_dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };

            let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 0.0,
                dpiY: 0.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };

            let rt = renderer
                .d2d_factory
                .CreateDCRenderTarget(&rt_props)
                .expect("Failed to create DC render target");
            let rc = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };

            // Test 1: 12-hour format "12:47 AM" with date "Monday, October 6"
            {
                let mut bits_12h: *mut std::ffi::c_void = std::ptr::null_mut();
                let dib_12h =
                    CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits_12h, None, 0)
                        .expect("DIB 12h");
                let old_bmp = SelectObject(mem_dc, dib_12h.into());
                rt.BindDC(mem_dc, &rc).expect("BindDC 12h");

                rt.BeginDraw();
                rt.Clear(Some(&COLOR_TRANSPARENT));

                let fill_brush = rt
                    .CreateSolidColorBrush(&NOTCH_BG_COLOR, None)
                    .expect("fill");
                let path = renderer.create_notch_geometry(&dims).expect("geometry");
                rt.FillGeometry(&path, &fill_brush, None);

                let layout = resolve_layout(&dims);
                if let ResolvedLayout::Expanded { components, .. } = &layout {
                    renderer
                        .render_expanded_content(
                            &rt,
                            components,
                            &dims,
                            "12:47 AM",
                            "Monday, October 6",
                        )
                        .expect("render 12h + date");
                }
                rt.EndDraw(None, None).expect("EndDraw 12h");

                let pixel_slice = std::slice::from_raw_parts(
                    bits_12h as *const u8,
                    (width * height * 4) as usize,
                );

                // Time region (y: 35..65, x: 200..400) has rendered primary text pixels
                let mut time_pixel_count = 0;
                for y in 35..65 {
                    for x in 200..400 {
                        let idx = ((y * width + x) * 4) as usize;
                        let r = pixel_slice[idx + 2];
                        if r > 20 {
                            time_pixel_count += 1;
                        }
                    }
                }
                assert!(
                    time_pixel_count > 30,
                    "Expanded 12h time text must be rendered"
                );

                // Date region (y: 70..95, x: 180..420) has rendered secondary text pixels
                let mut date_pixel_count = 0;
                for y in 70..95 {
                    for x in 180..420 {
                        let idx = ((y * width + x) * 4) as usize;
                        let r = pixel_slice[idx + 2];
                        if r > 20 {
                            date_pixel_count += 1;
                        }
                    }
                }
                assert!(date_pixel_count > 30, "Expanded date text must be rendered");

                SelectObject(mem_dc, old_bmp);
                let _ = DeleteObject(dib_12h.into());
            }

            // Test 2: 24-hour format "17:42" with date "Monday, October 6"
            {
                let mut bits_24h: *mut std::ffi::c_void = std::ptr::null_mut();
                let dib_24h =
                    CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits_24h, None, 0)
                        .expect("DIB 24h");
                let old_bmp = SelectObject(mem_dc, dib_24h.into());
                rt.BindDC(mem_dc, &rc).expect("BindDC 24h");

                rt.BeginDraw();
                rt.Clear(Some(&COLOR_TRANSPARENT));

                let fill_brush = rt
                    .CreateSolidColorBrush(&NOTCH_BG_COLOR, None)
                    .expect("fill");
                let path = renderer.create_notch_geometry(&dims).expect("geometry");
                rt.FillGeometry(&path, &fill_brush, None);

                let layout = resolve_layout(&dims);
                if let ResolvedLayout::Expanded { components, .. } = &layout {
                    renderer
                        .render_expanded_content(
                            &rt,
                            components,
                            &dims,
                            "17:42",
                            "Monday, October 6",
                        )
                        .expect("render 24h + date");
                }
                rt.EndDraw(None, None).expect("EndDraw 24h");

                let pixel_slice = std::slice::from_raw_parts(
                    bits_24h as *const u8,
                    (width * height * 4) as usize,
                );

                // Time region has rendered 24h text pixels
                let mut time_pixel_count = 0;
                for y in 35..65 {
                    for x in 200..400 {
                        let idx = ((y * width + x) * 4) as usize;
                        let r = pixel_slice[idx + 2];
                        if r > 20 {
                            time_pixel_count += 1;
                        }
                    }
                }
                assert!(
                    time_pixel_count > 25,
                    "Expanded 24h time text must be rendered"
                );

                SelectObject(mem_dc, old_bmp);
                let _ = DeleteObject(dib_24h.into());
            }

            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);
        }
    }

    #[test]
    fn test_collapsed_state_regression_unmodified() {
        let dims = NotchDimensions::from_state_and_dpi(NotchState::Collapsed, 96);
        assert_eq!(dims.width, 220);
        assert_eq!(dims.height, 32);
        assert_eq!(dims.curvature.bottom_radius, 14.0);
        assert_eq!(dims.curvature.top_transition_radius, 6.0);
        assert_eq!(dims.curvature.top_transition_height, 6.0);

        let layout = resolve_layout(&dims);
        match layout {
            ResolvedLayout::Collapsed { components, bounds } => {
                assert_eq!(bounds.width(), 220.0);
                assert_eq!(bounds.height(), 32.0);
                assert!(components.content_bounds.width() > 0.0);
                assert!(components.clock_bounds.width() > 0.0);
            }
            _ => panic!("Expected ResolvedLayout::Collapsed"),
        }
    }
}
