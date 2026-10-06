# Nott

Nott is a lightweight, ultra-efficient native Windows notch application built in Rust with native Win32 and Direct2D.

## Status

- **Phase 1 — Foundation**: COMPLETE (Stable borderless layered Win32 window, top-center positioning, DPI awareness, sub-pixel hit testing, clean lifecycle and shutdown).

---

## Architectural Pipeline

```
Win32 OS Messages (WM_MOUSEACTIVATE, WM_NCHITTEST, WM_DPICHANGED, WM_DISPLAYCHANGE)
   ↓
Native Event Handling (window.rs)
   ↓
DPI & Geometry Calculations (config.rs)
   ↓
Direct2D Renderer (renderer.rs)
   ↓
Desktop Window Manager Blit (UpdateLayeredWindow into 32-bit premultiplied ARGB DIB)
```

---

## Technical Specifications

- **Window Styles**: `WS_POPUP`
- **Extended Styles**: `WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`
- **Reference Geometry (@ 96 DPI)**:
  - Width: 220 px
  - Height: 32 px
  - Corner Radius: 14 px
  - Border Width: 1 px
- **Positioning**: Anchored top-center on the primary monitor (`((screen_width - width) / 2).max(0)`).
- **Hit Testing**: Sub-pixel anti-aliased rounded rectangle hit test returning `HTCLIENT` inside and `HTTRANSPARENT` outside (allowing clicks to pass through to underlying applications).
- **Idle CPU**: **0.0%** (blocks in `GetMessageW` event loop; zero polling, zero timers).

---

## Building and Testing

```powershell
# Format check
cargo fmt --check

# Check compilation
cargo check

# Run unit tests
cargo test

# Build debug & release executables
cargo build
cargo build --release
```

---

## Branding & Packaging Assets

Located under `assets/branding/`:
- `nott.ico`: Multi-resolution Windows icon (16x16, 32x32, 48x48, 64x64, 128x128, 256x256) ready for release packaging and executable embedding.
- `nott_icon_minimal.png` (from `nott.png`): Primary official icon featuring the minimalist hardware-notch cutout.
- `nott_icon_wordmark.png` (from `nott2.png`): Alternative brand variation featuring the embossed "Nott" wordmark inside the notch cutout.
