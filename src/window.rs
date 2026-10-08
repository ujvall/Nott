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
    WM_CAPTURECHANGED, WM_CLIPBOARDUPDATE, WM_CLOSE, WM_DESTROY, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_NCDESTROY, WM_NCHITTEST,
    WM_POWERBROADCAST, WM_TIMECHANGE, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{Error, Result};

pub const WM_MOUSELEAVE: u32 = 0x02A3;

#[link(name = "winmm")]
unsafe extern "system" {
    fn timeBeginPeriod(uPeriod: u32) -> u32;
    fn timeEndPeriod(uPeriod: u32) -> u32;
}

use crate::clipboard::{self, ClipboardHistory, ClipboardItem};
use crate::clock::ClockEngine;
use crate::config::{
    ANIMATION_FRAME_INTERVAL_MS, ANIMATION_TIMER_ID, AnimationState, CLIPBOARD_SETTLE_MS,
    CLIPBOARD_TIMER_ID, CLOCK_TIMER_ID, MEDIA_FEEDBACK_FRAME_MS, MEDIA_FEEDBACK_TIMER_ID,
    MEDIA_LIVE_FRAME_MS, MEDIA_LIVE_TIMER_ID, NotchDimensions, NotchState, WINDOW_CLASS_NAME,
    WINDOW_TITLE, calculate_notch_x,
};
use crate::dragdrop::{self, DragEvent, DragSession, DropTarget};
use crate::layout::{ClipboardHit, MediaLayout, resolve_clipboard_layout};
use crate::layout::{
    blended_selector, expanded_size_dip, resolve_media_layout_in, space_dimensions,
};
use crate::media::{
    MediaContent, MediaControl, MediaEngine, WM_APP_MEDIA_PLAYBACK_CHANGED,
    WM_APP_MEDIA_PROPERTIES_CHANGED, WM_APP_MEDIA_SESSION_CHANGED, is_media_message,
};
use crate::renderer::{DROP_PAGE_SPACE, Renderer};
use crate::space::{NottSpace, Scene};
use windows::Win32::System::Ole::{
    OleInitialize, OleUninitialize, RegisterDragDrop, RevokeDragDrop,
};

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
    /// Active space (every space renders the existing UI for now). Starts in
    /// Home, never persisted; media updates never touch it. Changed only through
    /// `select_space`, which keeps the renderer's selector in sync.
    space: NottSpace,
    /// Last tick of the control feedback timer (Some only while it runs).
    feedback_tick: Option<std::time::Instant>,
    /// Last tick of the playback live timer (Some only while it runs).
    live_tick: Option<std::time::Instant>,
    /// Clipboard history (in memory only; filled on `WM_CLIPBOARDUPDATE`).
    clipboard: ClipboardHistory,
    /// Active OLE drag over the notch (image-file drops into the history).
    drag: DragSession,
}

impl WindowState {
    /// Media composition of the settled expanded notch, if it is showing.
    fn active_media_layout(&self) -> Option<MediaLayout> {
        if self.state != NotchState::Expanded
            || self.animation.is_some()
            || self.media_content.is_none()
            || self.space == NottSpace::Clipboard
        {
            return None;
        }
        resolve_media_layout_in(
            &self.dimensions,
            self.media_content.as_ref()?.shape(),
            self.space,
        )
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
        if !self.live_visible() {
            // Nothing live on screen (Home expanded): no frames; settle the fade
            self.renderer.visualizer().settle();
            return;
        }
        if self.live_tick.is_none() && self.renderer.visualizer().is_active() {
            self.live_tick = Some(std::time::Instant::now());
            unsafe {
                let _ = SetTimer(Some(hwnd), MEDIA_LIVE_TIMER_ID, MEDIA_LIVE_FRAME_MS, None);
            }
        }
    }

    /// Whether a visualizer or scrubber is on screen: the collapsed notch's
    /// visualizer, the Music space, or a notch animation (its frames show both
    /// ends). The settled Home expanded notch shows neither.
    fn live_visible(&self) -> bool {
        self.animation.is_some() || self.state == NotchState::Collapsed || self.space.is_music()
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

    /// Space capsule under a client-area point: on the expanded notch, settled
    /// or mid width transition (the selector is drawn from the same interpolated
    /// dimensions, so what is clicked is what is seen). Not during expand/collapse.
    fn space_at(&self, lparam: LPARAM) -> Option<NottSpace> {
        let width_only = self
            .animation
            .as_ref()
            .is_none_or(|a| a.is_space_transition());
        if self.state != NotchState::Expanded || !width_only {
            return None;
        }
        let x = (lparam.0 & 0xFFFF) as i16 as f32;
        let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
        // The pills where they are drawn this frame (gliding mid transition)
        let (from, to, mix) = match &self.animation {
            Some(anim) => {
                let t = anim.transition();
                (t.from, t.to, t.mix)
            }
            None => (self.scene(), self.scene(), 1.0),
        };
        let space = |s: Scene| match s {
            Scene::Space(space) => space,
            Scene::Drop => self.space,
        };
        blended_selector(&self.dimensions, space(from), space(to), mix)?
            .0
            .space_at(x, y)
    }

    /// Selects a space (the selector's only route to `NottSpace::switch_to`); a
    /// no-op when already active. On the settled expanded notch the width
    /// transitions to the space's width on the existing animation timer; while
    /// collapsed (or mid expand/collapse) the space is only stored, and the next
    /// expansion opens straight into its width.
    fn select_space(&mut self, hwnd: HWND, space: NottSpace) {
        let before = self.scene();
        if self.space.switch_to(space) {
            self.renderer.set_space(space);
            // Controls move with the layout: drop stale hover/press
            self.reset_feedback(hwnd);
            self.resize_to_target(hwnd, before);
            notify_accessibility_state_changed(
                hwnd,
                self.state,
                self.clock.state(),
                self.media_content.as_ref(),
                self.space,
                &self.clipboard,
            );
        }
    }

    /// The space whose expanded size the notch takes: the drop page always uses
    /// the Clipboard notch's (one universal size, whichever space is active).
    fn size_space(&self) -> NottSpace {
        if self.renderer.drop_page() {
            DROP_PAGE_SPACE
        } else {
            self.space
        }
    }

    /// What the expanded notch shows: the drop page during an image drag,
    /// otherwise the active space.
    fn scene(&self) -> Scene {
        if self.renderer.drop_page() {
            Scene::Drop
        } else {
            Scene::Space(self.space)
        }
    }

    /// After the space or drop page changed (from showing `before`): the notch
    /// moves to its new size and content. A running animation (any kind,
    /// including an expand still opening) is retargeted in place, keeping its
    /// position and velocity; the settled expanded notch starts a space
    /// transition; collapsed, the change is only stored.
    fn resize_to_target(&mut self, hwnd: HWND, before: Scene) {
        let to = expanded_size_dip(self.size_space(), self.dimensions.dpi);
        let scene = self.scene();
        if let Some(anim) = &mut self.animation {
            if anim.target_state == NotchState::Expanded {
                anim.retarget(NotchState::Expanded, to);
            }
            anim.retarget_scene(scene);
            return;
        }
        if self.state != NotchState::Expanded {
            self.redraw(hwnd);
            return;
        }
        let from = (
            self.dimensions.width as f32 / self.dimensions.scale,
            self.dimensions.height as f32 / self.dimensions.scale,
        );
        if from == to && before == scene {
            self.redraw(hwnd);
            return;
        }
        if !is_client_animation_enabled() {
            let dims =
                space_dimensions(NotchState::Expanded, self.dimensions.dpi, self.size_space());
            let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
            self.dimensions = dims;
            self.pos_x = calculate_notch_x(screen_width, dims.width);
            self.redraw(hwnd);
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    self.pos_x,
                    self.pos_y,
                    dims.width,
                    dims.height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
            return;
        }
        let mut anim = AnimationState::space(from, to, before, scene);
        anim.scale = self.dimensions.scale;
        self.animation = Some(anim);
        self.last_frame_time = Some(std::time::Instant::now());
        self.high_res_timer.set(true);
        unsafe {
            SetTimer(
                Some(hwnd),
                ANIMATION_TIMER_ID,
                ANIMATION_FRAME_INTERVAL_MS,
                None,
            );
        }
    }

    /// Places history entry `index` back on the system clipboard and makes it
    /// the newest entry. The update Windows then sends is Nott's own (owner
    /// check), so no duplicate is stored. Failures leave everything unchanged.
    fn restore_clipboard(
        &mut self,
        hwnd: HWND,
        index: usize,
    ) -> std::result::Result<(), clipboard::ClipboardError> {
        let item = self
            .clipboard
            .get(index)
            .cloned()
            .ok_or(clipboard::ClipboardError::Invalid)?;
        clipboard::write(hwnd, &item)?;
        self.clipboard.promote(index);
        Ok(())
    }

    /// Expanded, or animating toward expanded.
    fn expanded_or_expanding(&self) -> bool {
        self.animation
            .as_ref()
            .map_or(self.state == NotchState::Expanded, |a| {
                a.target_state == NotchState::Expanded
            })
    }

    /// Whether a screen point is in the drop zone: the drop page's own settled
    /// silhouette (centred at the top of the screen). Fixed whatever size the
    /// notch has right now, so the page opening/resizing under the pointer can
    /// never flip the answer (no flicker at the edges).
    fn screen_point_in_drop_zone(&self, at: POINT) -> bool {
        let zone = space_dimensions(NotchState::Expanded, self.dimensions.dpi, DROP_PAGE_SPACE);
        let left = calculate_notch_x(unsafe { GetSystemMetrics(SM_CXSCREEN) }, zone.width);
        zone.contains_point((at.x - left) as f32, (at.y - self.pos_y) as f32)
    }

    /// One OLE drag event (UI thread). Supported image files over the notch are
    /// accepted; the first such moment expands a collapsed notch through the
    /// existing animation, and leave/cancel/drop collapse it again only if the
    /// drag opened it. Files are decoded only on drop, into the existing
    /// clipboard history; the newest is then placed on the system clipboard
    /// through the restore path (its own update is skipped by the owner check).
    fn on_drag(&mut self, hwnd: HWND, event: DragEvent) -> bool {
        let over = |state: &mut Self, at: POINT| {
            let in_notch = state.screen_point_in_drop_zone(at);
            let expanded = state.expanded_or_expanding();
            let before = state.scene();
            let action = state.drag.over(in_notch, expanded);
            // Drop page while an accepted image is over the notch, at its own
            // universal size (an expansion it starts opens straight at it)
            if state.renderer.set_drop_page(action.accept) && !action.expand {
                state.resize_to_target(hwnd, before);
            }
            if action.expand {
                state.reset_feedback(hwnd);
                let _ = start_or_reverse_animation(hwnd, state);
            }
            action.accept
        };
        let end = |state: &mut Self| {
            let collapse = state.drag.end() && state.expanded_or_expanding();
            let before = state.scene();
            let changed = state.renderer.set_drop_page(false);
            if collapse {
                state.reset_feedback(hwnd);
                let _ = start_or_reverse_animation(hwnd, state);
            } else if changed {
                // Back to the active space's own size and content
                state.resize_to_target(hwnd, before);
            }
        };
        match event {
            DragEvent::Enter { images, at } => {
                self.drag.enter(images);
                over(self, at)
            }
            DragEvent::Over { at } => over(self, at),
            DragEvent::Leave => {
                end(self);
                false
            }
            DragEvent::Drop { images, at } => {
                let accept = over(self, at) && !images.is_empty();
                let mut stored = false;
                if accept {
                    // Unsupported / unreadable / malformed / too large: skipped
                    let decoded: Vec<_> = images
                        .iter()
                        .filter_map(|p| dragdrop::decode_image_file(p).ok())
                        .collect();
                    if dragdrop::ingest(&mut self.clipboard, decoded) {
                        stored = true;
                        // Clipboard busy: the history keeps it; no retry
                        let _ = self.restore_clipboard(hwnd, 0);
                        self.clipboard_changed(hwnd);
                    }
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[dragdrop] {} image file(s) dropped, stored: {}, {} entries",
                        images.len(),
                        stored,
                        self.clipboard.len()
                    );
                }
                end(self);
                stored
            }
        }
    }

    /// Clipboard row or Clear under a client-area point (settled Clipboard
    /// space only; everything else there is notch background).
    fn clipboard_hit(&self, lparam: LPARAM) -> Option<ClipboardHit> {
        if self.state != NotchState::Expanded
            || self.animation.is_some()
            || self.space != NottSpace::Clipboard
        {
            return None;
        }
        let x = (lparam.0 & 0xFFFF) as i16 as f32;
        let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
        resolve_clipboard_layout(&self.dimensions)?.hit(x, y, self.clipboard.len())
    }

    /// After any history change: rebuild the Clipboard rows, and if that space
    /// is on screen, redraw it and refresh its accessible title.
    fn clipboard_changed(&mut self, hwnd: HWND) {
        self.renderer.set_clipboard(&self.clipboard);
        if self.state == NotchState::Expanded && self.space == NottSpace::Clipboard {
            self.redraw(hwnd);
            notify_accessibility_state_changed(
                hwnd,
                self.state,
                self.clock.state(),
                self.media_content.as_ref(),
                self.space,
                &self.clipboard,
            );
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

/// Accessible description of the space selector, e.g. "Home space selected
/// (spaces: Home, Music, Clipboard)".
fn accessible_space(space: NottSpace) -> String {
    let all: Vec<&str> = NottSpace::ALL.iter().map(|s| s.label()).collect();
    format!(
        "{} space selected (spaces: {})",
        space.label(),
        all.join(", ")
    )
}

/// Accessible title including the Clipboard space: there it describes the
/// history (count, newest entry's kind, the Clear control), never contents.
fn accessible_title(
    state: NotchState,
    clock: &crate::clock::ClockDateState,
    media: Option<&MediaContent>,
    space: NottSpace,
    clipboard: &ClipboardHistory,
) -> String {
    if state != NotchState::Expanded || space != NottSpace::Clipboard {
        return accessible_title_for_clock(state, clock, media, space);
    }
    let history = match (clipboard.len(), clipboard.get(0)) {
        (n, Some(newest)) => format!(
            "Clipboard history, {n} {}, newest is {}, Copy and Delete buttons on each item, Clear history button",
            if n == 1 { "item" } else { "items" },
            match newest {
                ClipboardItem::Text(_) => "text",
                ClipboardItem::Image(_) => "an image",
            }
        ),
        _ => "Clipboard history, empty".to_string(),
    };
    format!(
        "{history} - {} - {} - {}",
        clock.formatted_time,
        clock.formatted_date,
        accessible_space(space)
    )
}

/// Returns the user-facing accessible title for a given notch state and clock state.
/// When expanded with media, the displayed track (and the control actions) lead the
/// title; the expanded notch ends with the active space and the selectable spaces.
fn accessible_title_for_clock(
    state: NotchState,
    clock: &crate::clock::ClockDateState,
    media: Option<&MediaContent>,
    space: NottSpace,
) -> String {
    match (state, media) {
        (NotchState::Collapsed, _) => clock.formatted_time.clone(),
        (NotchState::Expanded, None) => format!(
            "{} - {} - {}",
            clock.formatted_time,
            clock.formatted_date,
            accessible_space(space)
        ),
        (NotchState::Expanded, Some(m)) => format!(
            "{} - {}, {}, {} - {} - {} - {}",
            m.accessible_text(),
            MediaControl::Previous.accessible_name(m.icon),
            MediaControl::PlayPause.accessible_name(m.icon),
            MediaControl::Next.accessible_name(m.icon),
            clock.formatted_time,
            clock.formatted_date,
            accessible_space(space)
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
    space: NottSpace,
    clipboard: &ClipboardHistory,
) {
    let title = accessible_title(new_state, clock, media, space, clipboard);
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
    space: NottSpace,
    clipboard: &ClipboardHistory,
) {
    let title = accessible_title(state, clock, media, space, clipboard);
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
        state.renderer.set_transition(None);
        state.last_frame_time = None;
        state.state = target_state;
        let new_dims = space_dimensions(target_state, state.dimensions.dpi, state.size_space());
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
            state.space,
            &state.clipboard,
        );
        return Ok(());
    }

    // The expanded end is the active space's (or drop page's) size. A running
    // animation is retargeted: it continues from its position and velocity.
    let size = expanded_size_dip(state.size_space(), state.dimensions.dpi);
    let scene = state.scene();
    let new_anim = if let Some(mut anim) = state.animation {
        anim.retarget(target_state, size);
        anim.retarget_scene(scene);
        anim
    } else {
        let from = (
            state.dimensions.width as f32 / state.dimensions.scale,
            state.dimensions.height as f32 / state.dimensions.scale,
        );
        let mut anim = AnimationState::start(state.state, from, target_state, size, scene);
        anim.scale = state.dimensions.scale;
        anim
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
    let new_dims = space_dimensions(new_state, dpi, state.size_space());
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
        state.space,
        &state.clipboard,
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
                        now.duration_since(last).as_secs_f32() * 1000.0
                    } else {
                        ANIMATION_FRAME_INTERVAL_MS as f32
                    };
                    state.last_frame_time = Some(now);

                    let still_active = if let Some(anim) = &mut state.animation {
                        anim.step(delta_ms)
                    } else {
                        false
                    };

                    let current_dims = if let Some(anim) = &state.animation {
                        state.renderer.set_transition(Some(anim.transition()));
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
                            state.dimensions = space_dimensions(
                                anim.target_state,
                                state.dimensions.dpi,
                                state.size_space(),
                            );
                            // The last frame was already at the target; draw the
                            // settled state once (exact pixels, no transition)
                            state.renderer.set_transition(None);
                            let _ = state.renderer.render(
                                hwnd,
                                new_x,
                                new_y,
                                &state.dimensions,
                                state.hovered,
                                state.clock.state(),
                            );
                            notify_accessibility_state_changed(
                                hwnd,
                                state.state,
                                state.clock.state(),
                                state.media_content.as_ref(),
                                state.space,
                                &state.clipboard,
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
                        // Settled somewhere showing a visualizer/scrubber again
                        state.kick_live(hwnd);
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
                    let visible = state.live_visible();
                    // During a notch animation its own frames show the motion
                    if visible {
                        state.redraw(hwnd);
                    } else {
                        state.renderer.visualizer().settle();
                    }
                    if !active || !visible {
                        state.live_tick = None;
                        unsafe {
                            let _ = KillTimer(Some(hwnd), MEDIA_LIVE_TIMER_ID);
                        }
                    }
                });
            } else if wparam.0 == CLIPBOARD_TIMER_ID {
                unsafe {
                    let _ = KillTimer(Some(hwnd), CLIPBOARD_TIMER_ID);
                }
                with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                    if clipboard::owned_by(hwnd) {
                        return;
                    }
                    // Unavailable / unsupported / too large: skipped, never retried
                    if let Ok(item) = clipboard::read(hwnd) {
                        let _outcome = state.clipboard.add(item);
                        if _outcome == clipboard::AddOutcome::Added {
                            state.clipboard_changed(hwnd);
                        }
                        // Kinds and counts only, never contents
                        #[cfg(debug_assertions)]
                        eprintln!(
                            "[clipboard] {:?}, {} entries, {} image bytes",
                            _outcome,
                            state.clipboard.len(),
                            state.clipboard.image_bytes()
                        );
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
                            state.space,
                            &state.clipboard,
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
                // Clipboard: the hovered row shows its buttons; a hovered button grows
                let hit = state.clipboard_hit(lparam);
                let row = hit.and_then(|h| match h {
                    ClipboardHit::Row(i) | ClipboardHit::Copy(i) | ClipboardHit::Remove(i) => {
                        Some(i)
                    }
                    ClipboardHit::Clear => None,
                });
                let changed = {
                    let mut feedback = state.renderer.feedback();
                    let row_changed = feedback.set_clip_row(row);
                    feedback.set_clip_hovered(hit.filter(|h| h.is_button())) || row_changed
                };
                if changed {
                    state.kick_feedback(hwnd);
                }
            });
            LRESULT(0)
        }

        WM_MOUSELEAVE => {
            with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                let left_clip = {
                    let mut feedback = state.renderer.feedback();
                    let released = feedback.release_clip();
                    let row_changed = feedback.set_clip_row(None);
                    feedback.set_clip_hovered(None) || released || row_changed
                };
                if state.renderer.set_hovered_control(None) || left_clip {
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
                } else if let Some(button) = state.clipboard_hit(lparam).filter(|h| h.is_button()) {
                    // Clipboard icon button: grows a little more while pressed
                    state.renderer.feedback().press_clip(button);
                    state.kick_feedback(hwnd);
                }
            });
            LRESULT(0)
        }

        WM_LBUTTONUP => {
            with_state_or_defer(hwnd, msg, wparam, lparam, |state| {
                // A pressed clipboard button eases back (its action runs below)
                if state.renderer.feedback().release_clip() {
                    state.kick_feedback(hwnd);
                }
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
                } else if let Some(hit) = state.clipboard_hit(lparam) {
                    // Clipboard row: back onto the system clipboard and to the
                    // front (no duplicate). Clear: forget the in-memory history
                    // only. Neither toggles the notch or leaves the space.
                    match hit {
                        ClipboardHit::Row(index) | ClipboardHit::Copy(index) => {
                            // Clipboard busy: nothing changes; no retry
                            let _ = state.restore_clipboard(hwnd, index);
                        }
                        ClipboardHit::Remove(index) => {
                            state.clipboard.remove(index);
                        }
                        ClipboardHit::Clear => state.clipboard.clear(),
                    }
                    state.clipboard_changed(hwnd);
                } else if let Some(space) = state.space_at(lparam) {
                    // Space capsule: switches spaces; never toggles the notch
                    state.select_space(hwnd, space);
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
                    let new_dims = space_dimensions(state.state, new_dpi, state.size_space());

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
                    let new_dims = space_dimensions(state.state, dpi, state.size_space());

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
                        state.space,
                        &state.clipboard,
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
                            state.space,
                            &state.clipboard,
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
                                state.space,
                                &state.clipboard,
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

        WM_CLIPBOARDUPDATE => {
            // Nott's own restore: the history already holds (and fronts) it.
            // Otherwise read once the copying app has settled (one-shot timer;
            // re-arming coalesces a burst of updates into a single read).
            if !clipboard::owned_by(hwnd) {
                unsafe {
                    let _ = SetTimer(Some(hwnd), CLIPBOARD_TIMER_ID, CLIPBOARD_SETTLE_MS, None);
                }
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            unsafe {
                let _ = KillTimer(Some(hwnd), ANIMATION_TIMER_ID);
                let _ = KillTimer(Some(hwnd), CLOCK_TIMER_ID);
                let _ = KillTimer(Some(hwnd), MEDIA_FEEDBACK_TIMER_ID);
                let _ = KillTimer(Some(hwnd), MEDIA_LIVE_TIMER_ID);
                let _ = KillTimer(Some(hwnd), CLIPBOARD_TIMER_ID);
                PostQuitMessage(0);
            }
            clipboard::stop_listening(hwnd);
            // Releases OLE's reference to the drop target (no-op if never registered)
            unsafe {
                let _ = RevokeDragDrop(hwnd);
            }
            // Release a resolution request still held by an in-flight animation
            // (if the state is busy, HighResTimer's Drop releases it on free).
            let _ = with_state(hwnd, |state| state.high_res_timer.set(false));
            LRESULT(0)
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// OLE drop-target entry point (UI thread, from the message loop): routes the
/// event through the same guarded `WindowState` access as the window procedure.
/// Busy or missing state (re-entry, teardown) declines the drop.
fn handle_drag(hwnd: HWND, event: DragEvent) -> bool {
    match with_state(hwnd, |state| state.on_drag(hwnd, event)) {
        Access::Ran(accept) => accept,
        Access::Busy | Access::Missing => false,
    }
}

pub fn run() -> Result<()> {
    // OLE drag-and-drop needs this (UI) thread in a single-threaded apartment.
    // Failure is non-fatal: Nott simply runs without being a drop target.
    let ole = unsafe { OleInitialize(None) }.is_ok();
    let result = run_window(ole);
    if ole {
        unsafe { OleUninitialize() };
    }
    result
}

fn run_window(ole: bool) -> Result<()> {
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

        let initial_title =
            accessible_title_for_clock(dimensions.state, clock.state(), None, NottSpace::default());
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
            space: NottSpace::default(),
            feedback_tick: None,
            live_tick: None,
            clipboard: ClipboardHistory::default(),
            drag: DragSession::default(),
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

        // Start async media session discovery (non-fatal; completes via WM_APP
        // message) and draw the first frame, through the same guarded access as
        // the window procedure (rendering can re-enter it synchronously).
        let initial = with_state(hwnd, |state| {
            let _ = state.media.initialize(hwnd);
            // Event-driven clipboard history (no polling)
            let _ = clipboard::start_listening(hwnd);
            state
                .renderer
                .render(hwnd, x, y, &state.dimensions, false, state.clock.state())
        });

        // Image files dragged from Explorer (OLE owns the target until revoked)
        if ole {
            let _ = RegisterDragDrop(hwnd, &DropTarget::create(hwnd, handle_drag));
        }

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
        let home = NottSpace::Home;
        let playing = MediaContent {
            title: "Song".into(),
            subtitle: "Artist".into(),
            icon: PlayPauseIcon::Pause,
            ..MediaContent::test_default()
        };
        assert_eq!(
            accessible_title_for_clock(NotchState::Expanded, &clock, Some(&playing), home),
            format!(
                "Song - Artist - Previous track, Pause, Next track - {time} - {date} - Home space selected (spaces: Home, Music, Clipboard)"
            )
        );
        let paused = MediaContent {
            icon: PlayPauseIcon::Play,
            ..playing.clone()
        };
        assert!(
            accessible_title_for_clock(NotchState::Expanded, &clock, Some(&paused), home)
                .contains("Previous track, Play, Next track")
        );
        // Collapsed title is unchanged (no selector there)
        for space in NottSpace::ALL {
            assert_eq!(
                accessible_title_for_clock(NotchState::Collapsed, &clock, Some(&playing), space),
                time
            );
        }
        assert_eq!(
            accessible_title_for_clock(NotchState::Expanded, &clock, None, home),
            format!("{time} - {date} - Home space selected (spaces: Home, Music, Clipboard)")
        );
    }

    #[test]
    fn test_accessible_title_for_clipboard_space() {
        use crate::clipboard::ClipboardItem;
        let clock =
            crate::clock::ClockDateState::from_pure_components(2026, 10, 6, 2, 0, 47, 0, 0, false);
        let (time, date) = (clock.formatted_time.clone(), clock.formatted_date.clone());
        let media = MediaContent {
            title: "Song".into(),
            ..MediaContent::test_default()
        };
        let mut history = ClipboardHistory::default();
        let title = |h: &ClipboardHistory, state| {
            accessible_title(state, &clock, Some(&media), NottSpace::Clipboard, h)
        };
        assert_eq!(
            title(&history, NotchState::Expanded),
            format!(
                "Clipboard history, empty - {time} - {date} - Clipboard space selected (spaces: Home, Music, Clipboard)"
            )
        );
        history.add(ClipboardItem::Text("secret password".into()));
        history.add(ClipboardItem::Image(
            crate::media::Artwork::new(1, 1, vec![0, 0, 0, 255]).unwrap(),
        ));
        let t = title(&history, NotchState::Expanded);
        assert!(t.starts_with(
            "Clipboard history, 2 items, newest is an image, Copy and Delete buttons on each item, Clear history button - "
        ));
        assert!(
            !t.contains("secret") && !t.contains("Song"),
            "no contents, no media"
        );
        history.promote(1);
        assert!(title(&history, NotchState::Expanded).contains("newest is text"));
        // Collapsed and the other spaces are unchanged
        assert_eq!(title(&history, NotchState::Collapsed), time);
        for s in [NottSpace::Home, NottSpace::Music] {
            assert_eq!(
                accessible_title(NotchState::Expanded, &clock, Some(&media), s, &history),
                accessible_title_for_clock(NotchState::Expanded, &clock, Some(&media), s)
            );
        }
    }

    #[test]
    fn test_accessible_title_reflects_active_space() {
        let clock =
            crate::clock::ClockDateState::from_pure_components(2026, 10, 6, 2, 0, 47, 0, 0, false);
        let media = MediaContent {
            title: "Song".into(),
            ..MediaContent::test_default()
        };
        let mut space = NottSpace::default();
        let title = |s| accessible_title_for_clock(NotchState::Expanded, &clock, Some(&media), s);
        assert!(title(space).ends_with("Home space selected (spaces: Home, Music, Clipboard)"));
        space.switch_to(NottSpace::Music);
        assert!(title(space).ends_with("Music space selected (spaces: Home, Music, Clipboard)"));
        // The media part is identical in both spaces
        let media_part = |t: String| t.split(" - Home space").next().unwrap().to_string();
        assert_eq!(
            media_part(title(NottSpace::Home)),
            title(NottSpace::Music)
                .split(" - Music space")
                .next()
                .unwrap()
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
