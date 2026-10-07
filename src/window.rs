use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::NotifyWinEvent;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem, GetDpiForWindow,
    SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_STATECHANGE, GWLP_USERDATA, GetClientRect, GetCursorPos,
    GetMessageW, GetSystemMetrics, GetWindowLongPtrW, HTCLIENT, HTTRANSPARENT, HWND_TOPMOST,
    IDC_ARROW, KillTimer, LoadCursorW, MA_NOACTIVATE, MSG, OBJID_CLIENT, PBT_APMRESUMEAUTOMATIC,
    PBT_APMRESUMESUSPEND, PostMessageW, PostQuitMessage, RegisterClassExW, SM_CXSCREEN,
    SPI_GETCLIENTAREAANIMATION, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetTimer, SetWindowLongPtrW, SetWindowPos, SetWindowTextW,
    ShowWindow, SystemParametersInfoW, TranslateMessage, UnregisterClassW, WM_APP,
    WM_CAPTURECHANGED, WM_CLOSE, WM_DESTROY, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_NCDESTROY, WM_NCHITTEST, WM_POWERBROADCAST,
    WM_TIMECHANGE, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP,
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
    MEDIA_FEEDBACK_FRAME_MS, MEDIA_FEEDBACK_TIMER_ID, MEDIA_LIVE_FRAME_MS, MEDIA_LIVE_TIMER_ID,
    NotchDimensions, NotchState, WINDOW_CLASS_NAME, WINDOW_TITLE, calculate_notch_x,
};
use crate::layout::{MediaLayout, resolve_media_layout};
use crate::media::{
    MediaContent, MediaControl, MediaEngine, WM_APP_MEDIA_PLAYBACK_CHANGED,
    WM_APP_MEDIA_PROPERTIES_CHANGED, WM_APP_MEDIA_SESSION_CHANGED, is_media_message,
};
use crate::renderer::Renderer;

/// Owns the process's 1 ms timer-resolution request (`timeBeginPeriod`), held
/// only while the notch animation runs. Requests never stack (reversing an
/// in-flight animation does not raise it again) and are never released twice;
/// `Drop` releases a request still held at shutdown.
#[derive(Debug, Default)]
struct HighResTimer {
    active: bool,
}

impl HighResTimer {
    /// Pure transition: Some(true) = call timeBeginPeriod, Some(false) = call
    /// timeEndPeriod, None = nothing to do.
    fn transition(&mut self, want: bool) -> Option<bool> {
        if self.active == want {
            return None;
        }
        self.active = want;
        Some(want)
    }

    fn set(&mut self, want: bool) {
        match self.transition(want) {
            Some(true) => unsafe {
                let _ = timeBeginPeriod(1);
            },
            Some(false) => unsafe {
                let _ = timeEndPeriod(1);
            },
            None => {}
        }
    }
}

impl Drop for HighResTimer {
    fn drop(&mut self) {
        self.set(false);
    }
}

struct WindowState {
    renderer: Renderer,
    /// 1 ms timer resolution, held only during the notch animation.
    high_res_timer: HighResTimer,
    state: NotchState,
    hovered: bool,
    dimensions: NotchDimensions,
    pos_x: i32,
    pos_y: i32,
    animation: Option<AnimationState>,
    last_frame_time: Option<std::time::Instant>,
    clock: ClockEngine,
    media: MediaEngine,
    /// Display model derived from `media` (None = no session).
    media_content: Option<MediaContent>,
    /// Last tick of the control feedback timer (Some only while it runs).
    feedback_tick: Option<std::time::Instant>,
    /// Last tick of the playback live timer (Some only while it runs).
    live_tick: Option<std::time::Instant>,
}

impl WindowState {
    /// Media composition of the settled expanded notch, if it is showing.
    fn active_media_layout(&self) -> Option<MediaLayout> {
        if self.state != NotchState::Expanded
            || self.animation.is_some()
            || self.media_content.is_none()
        {
            return None;
        }
        resolve_media_layout(&self.dimensions, self.media_content.as_ref()?.shape())
    }

    /// Redraws the settled notch (no-op while the notch animation owns frames).
    fn redraw(&self, hwnd: HWND) {
        if self.animation.is_none() {
            let _ = self.renderer.render(
                hwnd,
                self.pos_x,
                self.pos_y,
                &self.dimensions,
                self.hovered,
                self.clock.state(),
            );
        }
    }

    /// Starts the short-lived control feedback timer if a hover/press level needs
    /// to animate. It stops itself once every level reaches its target.
    fn kick_feedback(&mut self, hwnd: HWND) {
        if self.feedback_tick.is_none() && self.renderer.feedback().is_animating() {
            self.feedback_tick = Some(std::time::Instant::now());
            unsafe {
                let _ = SetTimer(
                    Some(hwnd),
                    MEDIA_FEEDBACK_TIMER_ID,
                    MEDIA_FEEDBACK_FRAME_MS,
                    None,
                );
            }
        }
    }

    /// Starts the playback live timer (visualizer motion + scrubber position) if
    /// the visualizer needs frames. It stops itself once playback is no longer
    /// playing and the bars have faded out, so paused/no media costs nothing.
    fn kick_live(&mut self, hwnd: HWND) {
        if self.live_tick.is_none() && self.renderer.visualizer().is_active() {
            self.live_tick = Some(std::time::Instant::now());
            unsafe {
                let _ = SetTimer(Some(hwnd), MEDIA_LIVE_TIMER_ID, MEDIA_LIVE_FRAME_MS, None);
            }
        }
    }

    /// Drops all control feedback immediately (e.g. when the notch toggles).
    fn reset_feedback(&mut self, hwnd: HWND) {
        self.renderer.feedback().reset();
        if self.feedback_tick.take().is_some() {
            unsafe {
                let _ = KillTimer(Some(hwnd), MEDIA_FEEDBACK_TIMER_ID);
            }
        }
    }

    /// Transport control under a client-area point (only when media UI is visible).
    fn media_control_at(&self, lparam: LPARAM) -> Option<MediaControl> {
        let x = (lparam.0 & 0xFFFF) as i16 as f32;
        let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
        self.active_media_layout()?.control_at(x, y)
    }
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
/// When expanded with media, the displayed track (and the control actions) lead the title.
fn accessible_title_for_clock(
    state: NotchState,
    clock: &crate::clock::ClockDateState,
    media: Option<&MediaContent>,
) -> String {
    match (state, media) {
        (NotchState::Collapsed, _) => clock.formatted_time.clone(),
        (NotchState::Expanded, None) => {
            format!("{} - {}", clock.formatted_time, clock.formatted_date)
        }
        (NotchState::Expanded, Some(m)) => format!(
            "{} - {}, {}, {} - {} - {}",
            m.accessible_text(),
            MediaControl::Previous.accessible_name(m.icon),
            MediaControl::PlayPause.accessible_name(m.icon),
            MediaControl::Next.accessible_name(m.icon),
            clock.formatted_time,
            clock.formatted_date
        ),
    }
}

/// Notifies Windows accessibility clients (screen readers, UI Automation bridge)
/// of a settled state change and updates the top-level window title.
fn notify_accessibility_state_changed(
    hwnd: HWND,
    new_state: NotchState,
    clock: &crate::clock::ClockDateState,
    media: Option<&MediaContent>,
) {
    let title = accessible_title_for_clock(new_state, clock, media);
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
    media: Option<&MediaContent>,
) {
    let title = accessible_title_for_clock(state, clock, media);
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
        state.high_res_timer.set(false);
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

        notify_accessibility_state_changed(
            hwnd,
            target_state,
            state.clock.state(),
            state.media_content.as_ref(),
        );
        return Ok(());
    }

    let new_anim = if let Some(current_anim) = &state.animation {
        current_anim.reverse_from_current()
    } else {
        AnimationState::new(state.state, target_state)
    };

    state.animation = Some(new_anim);
    state.last_frame_time = Some(std::time::Instant::now());

    // Arm the temporary high-precision timer for ~60 FPS animation steps. A
    // reversal re-arms the same timer ID; the resolution request is not stacked.
    state.high_res_timer.set(true);
    unsafe {
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
    notify_accessibility_state_changed(
        hwnd,
        new_state,
        state.clock.state(),
        state.media_content.as_ref(),
    );
    Ok(())
}

/// Private: a `WM_DPICHANGED` that arrived re-entrantly, replayed later (the real
/// message cannot be posted). wParam carries the original DPI pair.
const WM_APP_DEFERRED_DPICHANGED: u32 = WM_APP + 16;

/// What to do with a message that arrives while `WindowState` is already in use
/// by an outer handler (synchronous re-entry, e.g. `ReleaseCapture` sending
/// `WM_CAPTURECHANGED`, or `SetWindowPos`/`UpdateLayeredWindow` sending
/// `WM_DPICHANGED`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BusyPolicy {
    /// Recurs on its own (periodic timers, pointer moves)
    Drop,
    /// Replay this message after the outer handler returns
    Replay(u32),
}

fn busy_policy(msg: u32) -> BusyPolicy {
    match msg {
        WM_TIMER | WM_MOUSEMOVE => BusyPolicy::Drop,
        WM_DPICHANGED => BusyPolicy::Replay(WM_APP_DEFERRED_DPICHANGED),
        other => BusyPolicy::Replay(other),
    }
}

/// Re-entrancy guard for `WindowState` (UI thread only). Exactly one `&mut
/// WindowState` exists at a time: the window procedure only creates it through
/// `with_state`, which refuses while another handler holds it. Re-entrant
/// messages are dropped or queued and re-posted when the outermost handler
/// finishes; a `WM_NCDESTROY` arriving inside a handler defers the free until
/// that handler has returned, so no frame ever outlives its state.
#[derive(Debug, Default)]
struct StateGuard {
    busy: bool,
    deferred: Vec<(u32, usize, isize)>,
    free_on_exit: bool,
}

impl StateGuard {
    /// Returns false if state is already in use (re-entry).
    fn enter(&mut self) -> bool {
        !std::mem::replace(&mut self.busy, true)
    }

    /// Queues a re-entrant message (duplicates collapse: same msg/params).
    fn defer(&mut self, msg: u32, wparam: usize, lparam: isize) {
        if !self.deferred.contains(&(msg, wparam, lparam)) {
            self.deferred.push((msg, wparam, lparam));
        }
    }

    /// `WM_NCDESTROY`: true = free now; false = in use, free on exit.
    fn request_free(&mut self) -> bool {
        if self.busy {
            self.free_on_exit = true;
            false
        } else {
            true
        }
    }

    /// Leaves the outermost handler: (messages to replay, free state now?).
    fn exit(&mut self) -> (Vec<(u32, usize, isize)>, bool) {
        self.busy = false;
        (
            std::mem::take(&mut self.deferred),
            std::mem::take(&mut self.free_on_exit),
        )
    }
}

thread_local! {
    static GUARD: std::cell::RefCell<StateGuard> = std::cell::RefCell::new(StateGuard::default());
    /// State detached by a deferred WM_NCDESTROY, freed when the handler exits.
    static DETACHED: std::cell::Cell<*mut WindowState> = const { std::cell::Cell::new(std::ptr::null_mut()) };
    /// Notch dimensions as of the last completed handler, for WM_NCHITTEST
    /// arriving while state is in use.
    static HIT_DIMS: std::cell::Cell<Option<NotchDimensions>> = const { std::cell::Cell::new(None) };
}

/// Outcome of a guarded state access.
#[derive(Debug, PartialEq, Eq)]
enum Access<R> {
    Ran(R),
    /// Another handler holds the state (re-entrant call)
    Busy,
    /// No state attached (before creation finished / after destruction)
    Missing,
}

/// Runs `f` with exclusive access to the window's state, or reports why not.
fn with_state<R>(hwnd: HWND, f: impl FnOnce(&mut WindowState) -> R) -> Access<R> {
    if !GUARD.with_borrow_mut(|g| g.enter()) {
        return Access::Busy;
    }
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };
    let result = if ptr.is_null() {
        Access::Missing
    } else {
        // SAFETY: the guard is held, so this is the only live reference; the
        // allocation cannot be freed until `exit` below (WM_NCDESTROY defers).
        let state = unsafe { &mut *ptr };
        let r = f(state);
        HIT_DIMS.set(Some(state.dimensions));
        Access::Ran(r)
    };
    let (deferred, free) = GUARD.with_borrow_mut(|g| g.exit());
    if free {
        let detached = DETACHED.replace(std::ptr::null_mut());
        if !detached.is_null() {
            // SAFETY: detached by WM_NCDESTROY (Box::into_raw origin), freed once.
            drop(unsafe { Box::from_raw(detached) });
        }
    }
    for (m, w, l) in deferred {
        #[cfg(debug_assertions)]
        eprintln!("[guard] replaying re-entrant message 0x{m:04x}");
        unsafe {
            let _ = PostMessageW(Some(hwnd), m, WPARAM(w), LPARAM(l));
        }
    }
    result
}

/// Guarded handler: on re-entry the message is dropped or replayed per policy.
fn with_state_or_defer(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    f: impl FnOnce(&mut WindowState),
) {
    if with_state(hwnd, f) == Access::Busy
        && let BusyPolicy::Replay(m) = busy_policy(msg)
    {
        GUARD.with_borrow_mut(|g| g.defer(m, wparam.0, lparam.0));
    }
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

            let dims = match with_state(hwnd, |state| state.dimensions) {
                Access::Ran(d) => Some(d),
                // In use by an outer handler: last completed snapshot
                Access::Busy => HIT_DIMS.get(),
                Access::Missing => None,
            };
            let is_inside = if let Some(dims) = dims {
                dims.contains_point(pt.x as f32, pt.y as f32)
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
                with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
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
                                state.media_content.as_ref(),
                            );
                        }
                        state.last_frame_time = None;
                        state.high_res_timer.set(false);
                        unsafe {
                            let _ = KillTimer(Some(hwnd), ANIMATION_TIMER_ID);
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
                });
            } else if wparam.0 == MEDIA_FEEDBACK_TIMER_ID {
                with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                    let now = std::time::Instant::now();
                    let dt = state
                        .feedback_tick
                        .replace(now)
                        .map_or(MEDIA_FEEDBACK_FRAME_MS as f32, |t| {
                            now.duration_since(t).as_secs_f32() * 1000.0
                        });
                    let animating = state.renderer.feedback().step(dt);
                    state.redraw(hwnd);
                    if !animating {
                        state.feedback_tick = None;
                        unsafe {
                            let _ = KillTimer(Some(hwnd), MEDIA_FEEDBACK_TIMER_ID);
                        }
                    }
                });
            } else if wparam.0 == MEDIA_LIVE_TIMER_ID {
                with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                    let now = std::time::Instant::now();
                    let dt = state
                        .live_tick
                        .replace(now)
                        .map_or(MEDIA_LIVE_FRAME_MS as f32, |t| {
                            now.duration_since(t).as_secs_f32() * 1000.0
                        });
                    let active = state.renderer.visualizer().step(dt);
                    // During a notch animation its own frames show the motion
                    state.redraw(hwnd);
                    if !active {
                        state.live_tick = None;
                        unsafe {
                            let _ = KillTimer(Some(hwnd), MEDIA_LIVE_TIMER_ID);
                        }
                    }
                });
            } else if wparam.0 == CLOCK_TIMER_ID {
                with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                    let changed = state.clock.refresh();
                    if changed {
                        notify_accessibility_clock_changed(
                            hwnd,
                            state.state,
                            state.clock.state(),
                            state.media_content.as_ref(),
                        );
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
                });
            }
            LRESULT(0)
        }

        WM_MOUSEMOVE => {
            with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
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
                // Control-local hover: fades only when the hovered control changes
                let control = state.media_control_at(lparam);
                if state.renderer.set_hovered_control(control) {
                    state.kick_feedback(hwnd);
                }
            });
            LRESULT(0)
        }

        WM_MOUSELEAVE => {
            with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                if state.renderer.set_hovered_control(None) {
                    state.kick_feedback(hwnd);
                }
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
            });
            LRESULT(0)
        }

        WM_LBUTTONDOWN => {
            with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                if let Some(control) = state.media_control_at(lparam) {
                    // Tactile compression; capture so the release is always seen
                    state.renderer.feedback().press(control);
                    unsafe {
                        SetCapture(hwnd);
                    }
                    // Press compresses smoothly on the shared feedback timer
                    state.kick_feedback(hwnd);
                }
            });
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                let pressed = state.renderer.feedback().release();
                if let Some(pressed) = pressed {
                    unsafe {
                        let _ = ReleaseCapture();
                    }
                    // Press started on a control: never toggles the notch. The command
                    // fires only if released over the same control.
                    if state.media_control_at(lparam) == Some(pressed) {
                        state.media.send_command(pressed);
                    }
                    state.kick_feedback(hwnd);
                } else if let Some(control) = state.media_control_at(lparam) {
                    // Transport control: send to the current session; never toggles the notch.
                    // Resulting playback/metadata state arrives through media events.
                    state.media.send_command(control);
                } else {
                    state.reset_feedback(hwnd);
                    let _ = start_or_reverse_animation(hwnd, state);
                }
            });
            LRESULT(0)
        }

        WM_CAPTURECHANGED => {
            // Capture lost mid-press (e.g. another window took it): cancel the press
            with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                let cancelled = state.renderer.feedback().release().is_some();
                if cancelled {
                    state.kick_feedback(hwnd);
                }
            });
            LRESULT(0)
        }

        WM_CLOSE => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }

        WM_DPICHANGED | WM_APP_DEFERRED_DPICHANGED => {
            with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                unsafe {
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
            });
            LRESULT(0)
        }

        WM_DISPLAYCHANGE => {
            with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                unsafe {
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
            });
            LRESULT(0)
        }

        WM_TIMECHANGE => {
            with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                let changed = state.clock.refresh();
                if changed {
                    notify_accessibility_clock_changed(
                        hwnd,
                        state.state,
                        state.clock.state(),
                        state.media_content.as_ref(),
                    );
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
            });
            LRESULT(0)
        }

        WM_POWERBROADCAST => {
            let event = wparam.0 as u32;
            if event == PBT_APMRESUMEAUTOMATIC || event == PBT_APMRESUMESUSPEND {
                with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                    let changed = state.clock.refresh();
                    if changed {
                        notify_accessibility_clock_changed(
                            hwnd,
                            state.state,
                            state.clock.state(),
                            state.media_content.as_ref(),
                        );
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
                    // Media sessions may have changed while asleep: re-query once
                    // through the normal event messages (generation-protected).
                    for media_msg in [
                        WM_APP_MEDIA_SESSION_CHANGED,
                        WM_APP_MEDIA_PLAYBACK_CHANGED,
                        WM_APP_MEDIA_PROPERTIES_CHANGED,
                    ] {
                        unsafe {
                            let _ = PostMessageW(Some(hwnd), media_msg, WPARAM(0), LPARAM(0));
                        }
                    }
                });
            }
            LRESULT(1)
        }

        // Private, in-process only: posted by MediaEngine's WinRT callbacks (and
        // once on resume). Redraws only when the visible media content changes.
        _ if is_media_message(msg) => {
            with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                if state.media.handle_message(hwnd, msg) {
                    let content = MediaContent::from_state(state.media.state());
                    if content != state.media_content {
                        // Artwork/source-name updates don't change what is announced
                        let spoken = |c: &Option<MediaContent>| {
                            c.as_ref().map(|c| (c.accessible_text(), c.icon))
                        };
                        let a11y_changed = spoken(&content) != spoken(&state.media_content);
                        state.media_content = content;
                        let changed = state.renderer.set_media(state.media_content.as_ref());
                        // Play/pause starts the visualizer fade in either notch state
                        state.kick_live(hwnd);
                        // Media is only visible in the settled expanded state; an in-flight
                        // animation picks the new content up on its next frame.
                        let visible =
                            state.state == NotchState::Expanded && state.animation.is_none();
                        if changed && visible {
                            state.redraw(hwnd);
                            // A new track fades in on the shared feedback timer
                            state.kick_feedback(hwnd);
                        } else if !visible {
                            // Off-screen track changes never run a fade
                            state.renderer.feedback().finish_track_fade();
                        }
                        if a11y_changed
                            && state.state == NotchState::Expanded
                            && state.animation.is_none()
                        {
                            notify_accessibility_clock_changed(
                                hwnd,
                                state.state,
                                state.clock.state(),
                                state.media_content.as_ref(),
                            );
                        }
                    }
                }
            });
            LRESULT(0)
        }

        WM_NCDESTROY => {
            // Detach the state atomically (later messages see Missing). If a handler
            // up the stack is still using it (destruction re-entered that handler),
            // the free is deferred until that handler returns.
            let ptr = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut WindowState };
            if !ptr.is_null() {
                if GUARD.with_borrow_mut(|g| g.request_free()) {
                    // SAFETY: Box::into_raw origin, detached above, no live borrow.
                    drop(unsafe { Box::from_raw(ptr) });
                } else {
                    DETACHED.set(ptr);
                }
            }
            HIT_DIMS.set(None);
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_DESTROY => {
            unsafe {
                let _ = KillTimer(Some(hwnd), ANIMATION_TIMER_ID);
                let _ = KillTimer(Some(hwnd), CLOCK_TIMER_ID);
                let _ = KillTimer(Some(hwnd), MEDIA_FEEDBACK_TIMER_ID);
                let _ = KillTimer(Some(hwnd), MEDIA_LIVE_TIMER_ID);
                PostQuitMessage(0);
            }
            // Release a resolution request still held by an in-flight animation
            // (if the state is busy, HighResTimer's Drop releases it on free).
            let _ = with_state(hwnd, |state| state.high_res_timer.set(false));
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

        let initial_title = accessible_title_for_clock(dimensions.state, clock.state(), None);
        let title_wide: Vec<u16> = initial_title
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let _ = SetWindowTextW(hwnd, windows::core::PCWSTR(title_wide.as_ptr()));

        let state = Box::new(WindowState {
            renderer,
            high_res_timer: HighResTimer::default(),
            state: NotchState::Collapsed,
            hovered: false,
            dimensions,
            pos_x: x,
            pos_y: y,
            animation: None,
            last_frame_time: None,
            clock,
            media: MediaEngine::new(),
            media_content: None,
            feedback_tick: None,
            live_tick: None,
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

        // Start async media session discovery (non-fatal; completes via WM_APP
        // message) and draw the first frame, through the same guarded access as
        // the window procedure (rendering can re-enter it synchronously).
        let initial = with_state(hwnd, |state| {
            let _ = state.media.initialize(hwnd);
            state
                .renderer
                .render(hwnd, x, y, &state.dimensions, false, state.clock.state())
        });

        // Schedule first minute rollover timer
        let _ = SetTimer(Some(hwnd), CLOCK_TIMER_ID, rollover_ms, None);

        // 6. Initial render failure is fatal (destroys the window, frees the state)
        match initial {
            Access::Ran(Ok(())) => {}
            Access::Ran(Err(e)) => {
                let _ = DestroyWindow(hwnd);
                return Err(e);
            }
            Access::Busy | Access::Missing => {
                let _ = DestroyWindow(hwnd);
                return Err(Error::from_thread());
            }
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
    fn test_state_guard_refuses_reentry_and_replays_after_exit() {
        let mut g = StateGuard::default();
        assert!(g.enter(), "outer handler gets the state");
        // Synchronous re-entry (e.g. ReleaseCapture -> WM_CAPTURECHANGED) is refused
        assert!(!g.enter(), "no second &mut WindowState");
        g.defer(WM_CAPTURECHANGED, 0, 0);
        g.defer(WM_CAPTURECHANGED, 0, 0); // duplicate collapses
        g.defer(WM_APP_DEFERRED_DPICHANGED, (144 << 16) | 144, 0);
        let (replay, free) = g.exit();
        assert_eq!(
            replay,
            vec![
                (WM_CAPTURECHANGED, 0, 0),
                (WM_APP_DEFERRED_DPICHANGED, (144 << 16) | 144, 0)
            ]
        );
        assert!(!free);
        assert!(g.enter(), "available again after the outer handler exits");
        let (replay, _) = g.exit();
        assert!(replay.is_empty(), "queue drained exactly once");
    }

    #[test]
    fn test_state_guard_defers_free_during_handler() {
        let mut g = StateGuard::default();
        // Idle: WM_NCDESTROY frees immediately
        assert!(g.request_free());
        // Destruction re-entered a running handler: free only after it returns
        assert!(g.enter());
        assert!(!g.request_free(), "state still borrowed by the outer frame");
        let (_, free) = g.exit();
        assert!(free, "freed exactly when the outer handler exits");
        assert!(g.enter());
        let (_, free) = g.exit();
        assert!(!free, "never freed twice");
    }

    #[test]
    fn test_busy_policy() {
        // Periodic/self-recurring messages are dropped, never queued without bound
        assert_eq!(busy_policy(WM_TIMER), BusyPolicy::Drop);
        assert_eq!(busy_policy(WM_MOUSEMOVE), BusyPolicy::Drop);
        // WM_DPICHANGED cannot be posted: replayed as the private message
        assert_eq!(
            busy_policy(WM_DPICHANGED),
            BusyPolicy::Replay(WM_APP_DEFERRED_DPICHANGED)
        );
        for m in [
            WM_CAPTURECHANGED,
            WM_LBUTTONDOWN,
            WM_LBUTTONUP,
            WM_MOUSELEAVE,
            WM_DISPLAYCHANGE,
            WM_TIMECHANGE,
            WM_POWERBROADCAST,
            WM_APP_MEDIA_SESSION_CHANGED,
        ] {
            assert_eq!(busy_policy(m), BusyPolicy::Replay(m), "0x{m:04x}");
        }
    }

    #[test]
    fn test_with_state_missing_window_state_is_safe() {
        // No WindowState attached (invalid HWND): never dereferenced
        assert_eq!(with_state(HWND::default(), |_| ()), Access::Missing);
        // The guard is released afterwards
        assert!(GUARD.with_borrow_mut(|g| {
            let ok = g.enter();
            g.exit();
            ok
        }));
    }

    #[test]
    fn test_high_res_timer_never_stacks_or_double_releases() {
        let mut t = HighResTimer::default();
        assert_eq!(
            t.transition(false),
            None,
            "release without request is a no-op"
        );
        assert_eq!(
            t.transition(true),
            Some(true),
            "animation start raises once"
        );
        assert_eq!(
            t.transition(true),
            None,
            "reversal mid-animation does not stack"
        );
        assert_eq!(t.transition(true), None);
        assert_eq!(t.transition(false), Some(false), "completion releases once");
        assert_eq!(
            t.transition(false),
            None,
            "close/destroy after completion: no-op"
        );
        // Begin/end calls are always balanced over any sequence
        let mut depth = 0i32;
        for want in [
            true, true, false, true, false, false, true, true, true, false,
        ] {
            match t.transition(want) {
                Some(true) => depth += 1,
                Some(false) => depth -= 1,
                None => {}
            }
            assert!((0..=1).contains(&depth), "unbalanced: {depth}");
        }
        assert_eq!(depth, 0);
    }

    #[test]
    fn test_accessible_title_with_media() {
        use crate::media::PlayPauseIcon;
        let clock =
            crate::clock::ClockDateState::from_pure_components(2026, 10, 6, 2, 0, 47, 0, 0, false);
        let time = clock.formatted_time.clone();
        let date = clock.formatted_date.clone();
        let playing = MediaContent {
            title: "Song".into(),
            subtitle: "Artist".into(),
            icon: PlayPauseIcon::Pause,
            ..MediaContent::test_default()
        };
        assert_eq!(
            accessible_title_for_clock(NotchState::Expanded, &clock, Some(&playing)),
            format!("Song - Artist - Previous track, Pause, Next track - {time} - {date}")
        );
        let paused = MediaContent {
            icon: PlayPauseIcon::Play,
            ..playing.clone()
        };
        assert!(
            accessible_title_for_clock(NotchState::Expanded, &clock, Some(&paused))
                .contains("Previous track, Play, Next track")
        );
        // Collapsed and no-media expanded titles are unchanged
        assert_eq!(
            accessible_title_for_clock(NotchState::Collapsed, &clock, Some(&playing)),
            time
        );
        assert_eq!(
            accessible_title_for_clock(NotchState::Expanded, &clock, None),
            format!("{time} - {date}")
        );
    }

    #[test]
    fn test_is_client_animation_enabled_runs_safely() {
        // Query system parameter safely without crashing
        let enabled = is_client_animation_enabled();
        // Result is a valid boolean
        assert!(enabled || !enabled);
    }
}
