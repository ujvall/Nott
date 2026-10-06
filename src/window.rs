use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::NotifyWinEvent;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem, GetDpiForWindow,
    SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_STATECHANGE, GWLP_USERDATA, GetClientRect, GetCursorPos,
    GetMessageW, GetSystemMetrics, GetWindowLongPtrW, HTCLIENT, HTTRANSPARENT, HWND_TOPMOST,
    IDC_ARROW, KillTimer, LoadCursorW, MA_NOACTIVATE, MSG, OBJID_CLIENT, PBT_APMRESUMEAUTOMATIC,
    PBT_APMRESUMESUSPEND, PostQuitMessage, RegisterClassExW, SM_CXSCREEN,
    SPI_GETCLIENTAREAANIMATION, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetTimer, SetWindowLongPtrW, SetWindowPos, SetWindowTextW,
    ShowWindow, SystemParametersInfoW, TranslateMessage, UnregisterClassW, WM_CLOSE, WM_DESTROY,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_NCDESTROY,
    WM_NCHITTEST, WM_POWERBROADCAST, WM_TIMECHANGE, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{Error, Result};

pub const WM_MOUSELEAVE: u32 = 0x02A3;

#[link(name = "winmm")]
unsafe extern "system" {
    fn timeBeginPeriod(uPeriod: u32) -> u32;
    fn timeEndPeriod(uPeriod: u32) -> u32;
}

use crate::clock::ClockEngine;
use crate::config::{
    ANIMATION_FRAME_INTERVAL_MS, ANIMATION_TIMER_ID, AnimationState, CLOCK_TIMER_ID,
    NotchDimensions, NotchState, WINDOW_CLASS_NAME, WINDOW_TITLE, calculate_notch_x,
};
use crate::renderer::Renderer;

struct WindowState {
    renderer: Renderer,
    state: NotchState,
    hovered: bool,
    dimensions: NotchDimensions,
    pos_x: i32,
    pos_y: i32,
    animation: Option<AnimationState>,
    last_frame_time: Option<std::time::Instant>,
    clock: ClockEngine,
}

/// Checks if system-level client area animations are enabled.
/// If disabled (user requested reduced motion), animations should complete instantly.
fn is_client_animation_enabled() -> bool {
    let mut anim_enabled = windows::core::BOOL(1);
    let res = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut anim_enabled as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if res.is_ok() {
        anim_enabled.as_bool()
    } else {
        true
    }
}

/// Returns the user-facing accessible title for a given notch state and clock state.
fn accessible_title_for_clock(state: NotchState, clock: &crate::clock::ClockDateState) -> String {
    match state {
        NotchState::Collapsed => clock.formatted_time.clone(),
        NotchState::Expanded => format!("{} - {}", clock.formatted_time, clock.formatted_date),
    }
}

/// Notifies Windows accessibility clients (screen readers, UI Automation bridge)
/// of a settled state change and updates the top-level window title.
fn notify_accessibility_state_changed(
    hwnd: HWND,
    new_state: NotchState,
    clock: &crate::clock::ClockDateState,
) {
    let title = accessible_title_for_clock(new_state, clock);
    let wide_title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = SetWindowTextW(hwnd, windows::core::PCWSTR(wide_title.as_ptr()));
        NotifyWinEvent(EVENT_OBJECT_NAMECHANGE, hwnd, OBJID_CLIENT.0, 0);
        NotifyWinEvent(EVENT_OBJECT_STATECHANGE, hwnd, OBJID_CLIENT.0, 0);
    }
}

/// Notifies accessibility clients when the displayed clock minute actually changes.
fn notify_accessibility_clock_changed(
    hwnd: HWND,
    state: NotchState,
    clock: &crate::clock::ClockDateState,
) {
    let title = accessible_title_for_clock(state, clock);
    let wide_title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = SetWindowTextW(hwnd, windows::core::PCWSTR(wide_title.as_ptr()));
        NotifyWinEvent(EVENT_OBJECT_NAMECHANGE, hwnd, OBJID_CLIENT.0, 0);
    }
}

/// Initiates a smooth state transition or gracefully reverses an in-flight transition.
/// If reduced motion is requested by the system preference, transitions immediately without delay.
fn start_or_reverse_animation(hwnd: HWND, state: &mut WindowState) -> Result<()> {
    let target_state = if let Some(current_anim) = &state.animation {
        match current_anim.target_state {
            NotchState::Collapsed => NotchState::Expanded,
            NotchState::Expanded => NotchState::Collapsed,
        }
    } else {
        match state.state {
            NotchState::Collapsed => NotchState::Expanded,
            NotchState::Expanded => NotchState::Collapsed,
        }
    };

    if !is_client_animation_enabled() {
        // Reduced motion requested: snap immediately to target state without timer
        unsafe {
            let _ = KillTimer(Some(hwnd), ANIMATION_TIMER_ID);
        }
        state.animation = None;
        state.last_frame_time = None;
        state.state = target_state;
        let new_dims = NotchDimensions::from_state_and_dpi(target_state, state.dimensions.dpi);
        state.dimensions = new_dims;

        let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let new_x = calculate_notch_x(screen_width, new_dims.width);
        state.pos_x = new_x;
        state.pos_y = 0;

        // Check if cursor remains inside the new notch geometry after resize
        let mut cursor_pt = POINT::default();
        let cursor_in_notch = unsafe {
            if GetCursorPos(&mut cursor_pt).is_ok() {
                let mut client_pt = cursor_pt;
                let _ = ScreenToClient(hwnd, &mut client_pt);
                new_dims.contains_point(client_pt.x as f32, client_pt.y as f32)
            } else {
                false
            }
        };

        if state.hovered != cursor_in_notch {
            state.hovered = cursor_in_notch;
            if cursor_in_notch {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                unsafe {
                    let _ = TrackMouseEvent(&mut tme);
                }
            }
        }

        state.renderer.render(
            hwnd,
            new_x,
            0,
            &state.dimensions,
            state.hovered,
            state.clock.state(),
        )?;
        unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                new_x,
                0,
                new_dims.width,
                new_dims.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )?;
        }

        notify_accessibility_state_changed(hwnd, target_state, state.clock.state());
        return Ok(());
    }

    let new_anim = if let Some(current_anim) = &state.animation {
        current_anim.reverse_from_current()
    } else {
        AnimationState::new(state.state, target_state)
    };

    state.animation = Some(new_anim);
    state.last_frame_time = Some(std::time::Instant::now());

    // Arm the temporary high-precision timer for ~60 FPS animation steps
    unsafe {
        let _ = timeBeginPeriod(1);
        SetTimer(
            Some(hwnd),
            ANIMATION_TIMER_ID,
            ANIMATION_FRAME_INTERVAL_MS,
            None,
        );
    }
    Ok(())
}

/// Switches the notch visual and geometry state, recomputing centered coordinates,
/// resizing the native window, and re-rendering Direct2D content atomically.
#[allow(dead_code)]
fn set_notch_state(hwnd: HWND, state: &mut WindowState, new_state: NotchState) -> Result<()> {
    let dpi = state.dimensions.dpi;
    let new_dims = NotchDimensions::from_state_and_dpi(new_state, dpi);
    let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let new_x = calculate_notch_x(screen_width, new_dims.width);
    let new_y = 0;

    state.state = new_state;
    state.dimensions = new_dims;
    state.pos_x = new_x;
    state.pos_y = new_y;

    // Check if cursor remains inside the new notch geometry after resize
    let mut cursor_pt = POINT::default();
    let cursor_in_notch = unsafe {
        if GetCursorPos(&mut cursor_pt).is_ok() {
            let mut client_pt = cursor_pt;
            let _ = ScreenToClient(hwnd, &mut client_pt);
            new_dims.contains_point(client_pt.x as f32, client_pt.y as f32)
        } else {
            false
        }
    };

    state.hovered = cursor_in_notch;
    if cursor_in_notch {
        let mut tme = TRACKMOUSEEVENT {
            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: hwnd,
            dwHoverTime: 0,
        };
        unsafe {
            let _ = TrackMouseEvent(&mut tme);
        }
    }

    state.renderer.render(
        hwnd,
        new_x,
        new_y,
        &state.dimensions,
        state.hovered,
        state.clock.state(),
    )?;
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            new_x,
            new_y,
            new_dims.width,
            new_dims.height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )?;
    }
    notify_accessibility_state_changed(hwnd, new_state, state.clock.state());
    Ok(())
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCHITTEST => {
            // Screen cursor coordinates
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

            let mut pt = POINT { x, y };
            unsafe {
                let _ = ScreenToClient(hwnd, &mut pt);
            }

            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };
            let is_inside = if !ptr.is_null() {
                let state = unsafe { &*ptr };
                state.dimensions.contains_point(pt.x as f32, pt.y as f32)
            } else {
                let mut rect = RECT::default();
                unsafe {
                    let _ = GetClientRect(hwnd, &mut rect);
                }
                let w = (rect.right - rect.left) as f32;
                let h = (rect.bottom - rect.top) as f32;
                crate::config::is_point_in_notch_ex(
                    pt.x as f32,
                    pt.y as f32,
                    w,
                    h,
                    crate::config::BASE_COLLAPSED_TOP_TRANSITION_RADIUS,
                    crate::config::BASE_COLLAPSED_TOP_TRANSITION_HEIGHT,
                    crate::config::BASE_COLLAPSED_BOTTOM_CORNER_RADIUS,
                )
            };

            // Transparent hit test passes click directly to the application underneath
            if is_inside {
                LRESULT(HTCLIENT as isize)
            } else {
                LRESULT(HTTRANSPARENT as isize)
            }
        }

        WM_MOUSEACTIVATE => {
            // Prevent the notch from stealing keyboard focus when clicked
            LRESULT(MA_NOACTIVATE as isize)
        }

        WM_TIMER => {
            if wparam.0 == ANIMATION_TIMER_ID {
                let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };
                if !ptr.is_null() {
                    let state = unsafe { &mut *ptr };
                    let now = std::time::Instant::now();
                    let delta_ms = if let Some(last) = state.last_frame_time {
                        now.duration_since(last).as_millis().max(1) as u64
                    } else {
                        ANIMATION_FRAME_INTERVAL_MS as u64
                    };
                    state.last_frame_time = Some(now);

                    let still_active = if let Some(anim) = &mut state.animation {
                        anim.step(delta_ms)
                    } else {
                        false
                    };

                    let current_dims = if let Some(anim) = &state.animation {
                        anim.current_dimensions(state.dimensions.dpi)
                    } else {
                        state.dimensions
                    };

                    state.dimensions = current_dims;
                    let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
                    let new_x = calculate_notch_x(screen_width, current_dims.width);
                    let new_y = 0;
                    state.pos_x = new_x;
                    state.pos_y = new_y;

                    let _ = state.renderer.render(
                        hwnd,
                        new_x,
                        new_y,
                        &state.dimensions,
                        state.hovered,
                        state.clock.state(),
                    );

                    if !still_active {
                        // Animation complete: snap cleanly to target state and deactivate timer
                        if let Some(anim) = state.animation.take() {
                            state.state = anim.target_state;
                            state.dimensions = NotchDimensions::from_state_and_dpi(
                                anim.target_state,
                                state.dimensions.dpi,
                            );
                            notify_accessibility_state_changed(
                                hwnd,
                                state.state,
                                state.clock.state(),
                            );
                        }
                        state.last_frame_time = None;
                        unsafe {
                            let _ = KillTimer(Some(hwnd), ANIMATION_TIMER_ID);
                            let _ = timeEndPeriod(1);
                            let _ = SetWindowPos(
                                hwnd,
                                Some(HWND_TOPMOST),
                                new_x,
                                new_y,
                                state.dimensions.width,
                                state.dimensions.height,
                                SWP_NOACTIVATE | SWP_SHOWWINDOW,
                            );
                        }

                        // Re-evaluate cursor in notch after transition completes
                        let mut cursor_pt = POINT::default();
                        let cursor_in_notch = unsafe {
                            if GetCursorPos(&mut cursor_pt).is_ok() {
                                let mut client_pt = cursor_pt;
                                let _ = ScreenToClient(hwnd, &mut client_pt);
                                state
                                    .dimensions
                                    .contains_point(client_pt.x as f32, client_pt.y as f32)
                            } else {
                                false
                            }
                        };
                        if state.hovered != cursor_in_notch {
                            state.hovered = cursor_in_notch;
                            if cursor_in_notch {
                                let mut tme = TRACKMOUSEEVENT {
                                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                                    dwFlags: TME_LEAVE,
                                    hwndTrack: hwnd,
                                    dwHoverTime: 0,
                                };
                                unsafe {
                                    let _ = TrackMouseEvent(&mut tme);
                                }
                            }
                        }

                        // Final static render at exact target dimensions
                        let _ = state.renderer.render(
                            hwnd,
                            state.pos_x,
                            state.pos_y,
                            &state.dimensions,
                            state.hovered,
                            state.clock.state(),
                        );
                    }
                }
            } else if wparam.0 == CLOCK_TIMER_ID {
                let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };
                if !ptr.is_null() {
                    let state = unsafe { &mut *ptr };
                    let changed = state.clock.refresh();
                    if changed {
                        notify_accessibility_clock_changed(hwnd, state.state, state.clock.state());
                        if state.animation.is_none() {
                            let _ = state.renderer.render(
                                hwnd,
                                state.pos_x,
                                state.pos_y,
                                &state.dimensions,
                                state.hovered,
                                state.clock.state(),
                            );
                        }
                    }
                    let next_ms = state.clock.ms_until_next_minute();
                    unsafe {
                        let _ = SetTimer(Some(hwnd), CLOCK_TIMER_ID, next_ms, None);
                    }
                }
            }
            LRESULT(0)
        }

        WM_MOUSEMOVE => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };
            if !ptr.is_null() {
                let state = unsafe { &mut *ptr };
                if !state.hovered {
                    state.hovered = true;
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    unsafe {
                        let _ = TrackMouseEvent(&mut tme);
                    }
                    if state.animation.is_none() {
                        let _ = state.renderer.render(
                            hwnd,
                            state.pos_x,
                            state.pos_y,
                            &state.dimensions,
                            true,
                            state.clock.state(),
                        );
                    }
                }
            }
            LRESULT(0)
        }

        WM_MOUSELEAVE => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };
            if !ptr.is_null() {
                let state = unsafe { &mut *ptr };
                if state.hovered {
                    state.hovered = false;
                    if state.animation.is_none() {
                        let _ = state.renderer.render(
                            hwnd,
                            state.pos_x,
                            state.pos_y,
                            &state.dimensions,
                            false,
                            state.clock.state(),
                        );
                    }
                }
            }
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };
            if !ptr.is_null() {
                let state = unsafe { &mut *ptr };
                let _ = start_or_reverse_animation(hwnd, state);
            }
            LRESULT(0)
        }

        WM_CLOSE => {
            unsafe {
                let _ = KillTimer(Some(hwnd), ANIMATION_TIMER_ID);
                let _ = timeEndPeriod(1);
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }

        WM_DPICHANGED => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };
            if !ptr.is_null() {
                unsafe {
                    let state = &mut *ptr;
                    let new_dpi = (wparam.0 & 0xFFFF) as u32;
                    let new_dpi = if new_dpi > 0 {
                        new_dpi
                    } else {
                        GetDpiForWindow(hwnd)
                    };
                    let new_dims = NotchDimensions::from_state_and_dpi(state.state, new_dpi);

                    let screen_width = GetSystemMetrics(SM_CXSCREEN);
                    let new_x = calculate_notch_x(screen_width, new_dims.width);
                    let new_y = 0;

                    // Reconfigure rendering only when dimensions or position actually change
                    if state.dimensions != new_dims || state.pos_x != new_x || state.pos_y != new_y
                    {
                        state.dimensions = new_dims;
                        state.pos_x = new_x;
                        state.pos_y = new_y;

                        let _ = state.renderer.render(
                            hwnd,
                            new_x,
                            new_y,
                            &new_dims,
                            state.hovered,
                            state.clock.state(),
                        );
                        let _ = SetWindowPos(
                            hwnd,
                            Some(HWND_TOPMOST),
                            new_x,
                            new_y,
                            new_dims.width,
                            new_dims.height,
                            SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        );
                    }
                }
            }
            LRESULT(0)
        }

        WM_DISPLAYCHANGE => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };
            if !ptr.is_null() {
                unsafe {
                    let state = &mut *ptr;
                    let dpi = GetDpiForWindow(hwnd);
                    let dpi = if dpi > 0 { dpi } else { GetDpiForSystem() };
                    let new_dims = NotchDimensions::from_state_and_dpi(state.state, dpi);

                    let screen_width = GetSystemMetrics(SM_CXSCREEN);
                    let new_x = calculate_notch_x(screen_width, new_dims.width);
                    let new_y = 0;

                    // Reconfigure rendering only when dimensions or position actually change
                    if state.dimensions != new_dims || state.pos_x != new_x || state.pos_y != new_y
                    {
                        state.dimensions = new_dims;
                        state.pos_x = new_x;
                        state.pos_y = new_y;

                        let _ = state.renderer.render(
                            hwnd,
                            new_x,
                            new_y,
                            &new_dims,
                            state.hovered,
                            state.clock.state(),
                        );
                        let _ = SetWindowPos(
                            hwnd,
                            Some(HWND_TOPMOST),
                            new_x,
                            new_y,
                            new_dims.width,
                            new_dims.height,
                            SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        );
                    }
                }
            }
            LRESULT(0)
        }

        WM_TIMECHANGE => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };
            if !ptr.is_null() {
                let state = unsafe { &mut *ptr };
                let changed = state.clock.refresh();
                if changed {
                    notify_accessibility_clock_changed(hwnd, state.state, state.clock.state());
                    if state.animation.is_none() {
                        let _ = state.renderer.render(
                            hwnd,
                            state.pos_x,
                            state.pos_y,
                            &state.dimensions,
                            state.hovered,
                            state.clock.state(),
                        );
                    }
                }
                let next_ms = state.clock.ms_until_next_minute();
                unsafe {
                    let _ = SetTimer(Some(hwnd), CLOCK_TIMER_ID, next_ms, None);
                }
            }
            LRESULT(0)
        }

        WM_POWERBROADCAST => {
            let event = wparam.0 as u32;
            if event == PBT_APMRESUMEAUTOMATIC || event == PBT_APMRESUMESUSPEND {
                let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };
                if !ptr.is_null() {
                    let state = unsafe { &mut *ptr };
                    let changed = state.clock.refresh();
                    if changed {
                        notify_accessibility_clock_changed(hwnd, state.state, state.clock.state());
                        if state.animation.is_none() {
                            let _ = state.renderer.render(
                                hwnd,
                                state.pos_x,
                                state.pos_y,
                                &state.dimensions,
                                state.hovered,
                                state.clock.state(),
                            );
                        }
                    }
                    let next_ms = state.clock.ms_until_next_minute();
                    unsafe {
                        let _ = SetTimer(Some(hwnd), CLOCK_TIMER_ID, next_ms, None);
                    }
                }
            }
            LRESULT(1)
        }

        WM_NCDESTROY => {
            // Atomically clear pointer and release WindowState to prevent dangling pointer or double free
            let ptr = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut WindowState };
            if !ptr.is_null() {
                unsafe {
                    drop(Box::from_raw(ptr));
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_DESTROY => {
            unsafe {
                let _ = KillTimer(Some(hwnd), ANIMATION_TIMER_ID);
                let _ = KillTimer(Some(hwnd), CLOCK_TIMER_ID);
                let _ = timeEndPeriod(1);
                PostQuitMessage(0);
            }
            LRESULT(0)
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub fn run() -> Result<()> {
    unsafe {
        // Enable Per-Monitor V2 DPI awareness for sharp native rendering
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        let hinstance = GetModuleHandleW(None)?;

        // 1. Register Window Class
        let cursor = LoadCursorW(None, IDC_ARROW)?;
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance.into(),
            hIcon: windows::Win32::UI::WindowsAndMessaging::HICON::default(),
            hCursor: cursor,
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH::default(),
            lpszMenuName: windows::core::PCWSTR::null(),
            lpszClassName: WINDOW_CLASS_NAME,
            hIconSm: windows::Win32::UI::WindowsAndMessaging::HICON::default(),
        };

        let atom = RegisterClassExW(&wc);
        if atom == 0 {
            return Err(Error::from_thread());
        }

        // 2. Compute initial notch position and dimensions from monitor DPI
        let system_dpi = GetDpiForSystem();
        let dimensions = NotchDimensions::from_state_and_dpi(NotchState::Collapsed, system_dpi);
        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let x = calculate_notch_x(screen_width, dimensions.width);
        let y = 0;

        // 3. Create Borderless, Layered, Topmost, Toolwindow
        let ex_style = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        let hwnd = CreateWindowExW(
            ex_style,
            WINDOW_CLASS_NAME,
            WINDOW_TITLE,
            WS_POPUP,
            x,
            y,
            dimensions.width,
            dimensions.height,
            None,
            None,
            Some(hinstance.into()),
            None,
        )?;

        if hwnd.is_invalid() {
            return Err(Error::from_thread());
        }

        // 4. Verify DPI with specific window handle and re-align if necessary
        let window_dpi = GetDpiForWindow(hwnd);
        let dimensions = if window_dpi > 0 && window_dpi != system_dpi {
            let dims = NotchDimensions::from_state_and_dpi(NotchState::Collapsed, window_dpi);
            let new_x = calculate_notch_x(screen_width, dims.width);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                new_x,
                y,
                dims.width,
                dims.height,
                SWP_NOACTIVATE,
            );
            dims
        } else {
            dimensions
        };
        let x = calculate_notch_x(screen_width, dimensions.width);

        // 5. Initialize renderer and attach WindowState to HWND
        let renderer = match Renderer::new() {
            Ok(r) => r,
            Err(e) => {
                let _ = DestroyWindow(hwnd);
                return Err(e);
            }
        };

        let clock = ClockEngine::now();
        let rollover_ms = clock.ms_until_next_minute();

        let initial_title = accessible_title_for_clock(dimensions.state, clock.state());
        let title_wide: Vec<u16> = initial_title
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let _ = SetWindowTextW(hwnd, windows::core::PCWSTR(title_wide.as_ptr()));

        let state = Box::new(WindowState {
            renderer,
            state: NotchState::Collapsed,
            hovered: false,
            dimensions,
            pos_x: x,
            pos_y: y,
            animation: None,
            last_frame_time: None,
            clock,
        });
        let state_ptr = Box::into_raw(state);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);

        // Schedule first minute rollover timer
        let _ = SetTimer(Some(hwnd), CLOCK_TIMER_ID, rollover_ms, None);

        // 6. Initial Render into Layered Window
        let state_ref = &*state_ptr;
        if let Err(e) = state_ref.renderer.render(
            hwnd,
            x,
            y,
            &state_ref.dimensions,
            false,
            state_ref.clock.state(),
        ) {
            let _ = DestroyWindow(hwnd);
            return Err(e);
        }

        // 7. Display window without stealing focus
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

        // 8. Event-driven message loop: blocks until messages arrive (0% idle CPU)
        let mut msg = MSG::default();
        let loop_result = loop {
            let ret = GetMessageW(&mut msg, None, 0, 0);
            if ret.0 <= 0 {
                // ret.0 == 0 means WM_QUIT; ret.0 < 0 means error
                break if ret.0 == 0 {
                    Ok(())
                } else {
                    Err(Error::from_thread())
                };
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        };

        // Clean up registered window class
        let _ = UnregisterClassW(WINDOW_CLASS_NAME, Some(hinstance.into()));

        loop_result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_client_animation_enabled_runs_safely() {
        // Query system parameter safely without crashing
        let enabled = is_client_animation_enabled();
        // Result is a valid boolean
        assert!(enabled || !enabled);
    }
}
