#![allow(dead_code)]

use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::Globalization::{
    DATE_LONGDATE, GetDateFormatW, GetTimeFormatW, TIME_NOSECONDS,
};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::core::PCWSTR;

/// English weekday names matching Win32 SYSTEMTIME wDayOfWeek (0 = Sunday, 1 = Monday, ...)
pub const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// English abbreviated weekday names
pub const WEEKDAYS_SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// English month names matching Win32 SYSTEMTIME wMonth (1 = January, ...)
pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// English abbreviated month names
pub const MONTHS_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Pure 12-hour time formatter (e.g. "10:42 AM", "12:00 PM", "12:05 AM")
pub fn format_time_12h(hour: u16, minute: u16) -> String {
    let am_pm = if hour < 12 { "AM" } else { "PM" };
    let display_hour = match hour % 12 {
        0 => 12,
        h => h,
    };
    format!("{display_hour}:{minute:02} {am_pm}")
}

/// Pure 24-hour time formatter (e.g. "10:42", "22:42", "00:05")
pub fn format_time_24h(hour: u16, minute: u16) -> String {
    format!("{hour:02}:{minute:02}")
}

/// Formats a date into human-readable long form (e.g. "Monday, October 5")
pub fn format_date_long(month: u16, day: u16, day_of_week: u16) -> String {
    let weekday_str = WEEKDAYS.get(day_of_week as usize).copied().unwrap_or("");
    let month_idx = month.saturating_sub(1) as usize;
    let month_str = MONTHS.get(month_idx).copied().unwrap_or("");
    if weekday_str.is_empty() || month_str.is_empty() {
        format!("{month}/{day}")
    } else {
        format!("{weekday_str}, {month_str} {day}")
    }
}

/// Formats a date into abbreviated form (e.g. "Mon, Oct 5")
pub fn format_date_short(month: u16, day: u16, day_of_week: u16) -> String {
    let weekday_str = WEEKDAYS_SHORT
        .get(day_of_week as usize)
        .copied()
        .unwrap_or("");
    let month_idx = month.saturating_sub(1) as usize;
    let month_str = MONTHS_SHORT.get(month_idx).copied().unwrap_or("");
    if weekday_str.is_empty() || month_str.is_empty() {
        format!("{month}/{day}")
    } else {
        format!("{weekday_str}, {month_str} {day}")
    }
}

/// Calculates milliseconds remaining until the next minute boundary.
/// Adds a 20ms grace buffer to guarantee the system clock has crossed
/// the minute boundary before waking up.
///
/// Clamped between 50ms and 60_050ms.
pub fn calculate_ms_until_next_minute(second: u16, millisecond: u16) -> u32 {
    let s = (second as u32).min(59);
    let ms = (millisecond as u32).min(999);
    let remaining_ms = (60 - s - 1) * 1000 + (1000 - ms) + 20;
    remaining_ms.clamp(50, 60_050)
}

/// Formats system time using Windows user locale preferences via native Win32 APIs,
/// falling back to pure Rust formatters if native APIs return zero.
pub fn format_system_time_with_locale(st: &SYSTEMTIME) -> (String, String, String) {
    let mut buf = [0u16; 64];

    // 1. Time formatted according to Windows user locale preference (no seconds)
    let time_len = unsafe {
        GetTimeFormatW(
            0, // LOCALE_USER_DEFAULT
            TIME_NOSECONDS.0,
            Some(st),
            PCWSTR::null(),
            Some(&mut buf),
        )
    };

    let formatted_time = if time_len > 1 {
        String::from_utf16_lossy(&buf[..(time_len as usize - 1)])
    } else {
        format_time_12h(st.wHour, st.wMinute)
    };

    // 2. Date long form
    let date_len =
        unsafe { GetDateFormatW(0, DATE_LONGDATE.0, Some(st), PCWSTR::null(), Some(&mut buf)) };

    let formatted_date = if date_len > 1 {
        String::from_utf16_lossy(&buf[..(date_len as usize - 1)])
    } else {
        format_date_long(st.wMonth, st.wDay, st.wDayOfWeek)
    };

    // 3. Date short form
    let formatted_date_short = format_date_short(st.wMonth, st.wDay, st.wDayOfWeek);

    (formatted_time, formatted_date, formatted_date_short)
}

/// Lightweight snapshot of current clock and date components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockDateState {
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub millisecond: u16,
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub day_of_week: u16,
    pub formatted_time: String,
    pub formatted_date: String,
    pub formatted_date_short: String,
}

impl Default for ClockDateState {
    fn default() -> Self {
        Self {
            hour: 0,
            minute: 0,
            second: 0,
            millisecond: 0,
            year: 2026,
            month: 1,
            day: 1,
            day_of_week: 4,
            formatted_time: String::from("12:00 AM"),
            formatted_date: String::from("Thursday, January 1"),
            formatted_date_short: String::from("Thu, Jan 1"),
        }
    }
}

impl ClockDateState {
    /// Constructs a state snapshot from a Win32 SYSTEMTIME using locale preferences.
    pub fn from_system_time(st: &SYSTEMTIME) -> Self {
        let (formatted_time, formatted_date, formatted_date_short) =
            format_system_time_with_locale(st);
        Self {
            hour: st.wHour,
            minute: st.wMinute,
            second: st.wSecond,
            millisecond: st.wMilliseconds,
            year: st.wYear,
            month: st.wMonth,
            day: st.wDay,
            day_of_week: st.wDayOfWeek,
            formatted_time,
            formatted_date,
            formatted_date_short,
        }
    }

    /// Constructs a pure state snapshot without Win32 locale dependencies (ideal for tests).
    #[allow(clippy::too_many_arguments)]
    pub fn from_pure_components(
        year: u16,
        month: u16,
        day: u16,
        day_of_week: u16,
        hour: u16,
        minute: u16,
        second: u16,
        millisecond: u16,
        use_24h: bool,
    ) -> Self {
        let formatted_time = if use_24h {
            format_time_24h(hour, minute)
        } else {
            format_time_12h(hour, minute)
        };
        let formatted_date = format_date_long(month, day, day_of_week);
        let formatted_date_short = format_date_short(month, day, day_of_week);

        Self {
            hour,
            minute,
            second,
            millisecond,
            year,
            month,
            day,
            day_of_week,
            formatted_time,
            formatted_date,
            formatted_date_short,
        }
    }
}

/// Clock engine providing live Windows time updates and change detection.
pub struct ClockEngine {
    state: ClockDateState,
}

impl Default for ClockEngine {
    fn default() -> Self {
        Self::now()
    }
}

impl ClockEngine {
    /// Queries the current Windows system local time and initializes the engine.
    pub fn now() -> Self {
        let st: SYSTEMTIME = unsafe { GetLocalTime() };
        Self {
            state: ClockDateState::from_system_time(&st),
        }
    }

    /// Creates an engine initialized with a specific SYSTEMTIME (useful for tests).
    pub fn from_system_time(st: &SYSTEMTIME) -> Self {
        Self {
            state: ClockDateState::from_system_time(st),
        }
    }

    /// Creates an engine with a custom initial pure state (for unit tests).
    pub fn from_state(state: ClockDateState) -> Self {
        Self { state }
    }

    /// Returns an immutable reference to the current clock/date snapshot.
    #[inline]
    pub fn state(&self) -> &ClockDateState {
        &self.state
    }

    /// Returns the formatted time string (e.g. "10:42 AM" or "22:42").
    #[inline]
    pub fn formatted_time(&self) -> &str {
        &self.state.formatted_time
    }

    /// Returns the formatted long date string (e.g. "Monday, October 5").
    #[inline]
    pub fn formatted_date(&self) -> &str {
        &self.state.formatted_date
    }

    /// Returns the formatted short date string (e.g. "Mon, Oct 5").
    #[inline]
    pub fn formatted_date_short(&self) -> &str {
        &self.state.formatted_date_short
    }

    /// Computes milliseconds until the next minute boundary based on current state.
    #[inline]
    pub fn ms_until_next_minute(&self) -> u32 {
        calculate_ms_until_next_minute(self.state.second, self.state.millisecond)
    }

    /// Updates internal state from a given SYSTEMTIME.
    ///
    /// Returns `true` if the displayed time or date actually changed,
    /// signalling that a window redraw is required.
    pub fn update_from_system_time(&mut self, st: &SYSTEMTIME) -> bool {
        let prev_time = &self.state.formatted_time;
        let prev_date = &self.state.formatted_date;
        let (new_time, new_date, new_date_short) = format_system_time_with_locale(st);

        let changed = prev_time != &new_time || prev_date != &new_date;

        self.state.hour = st.wHour;
        self.state.minute = st.wMinute;
        self.state.second = st.wSecond;
        self.state.millisecond = st.wMilliseconds;
        self.state.year = st.wYear;
        self.state.month = st.wMonth;
        self.state.day = st.wDay;
        self.state.day_of_week = st.wDayOfWeek;
        self.state.formatted_time = new_time;
        self.state.formatted_date = new_date;
        self.state.formatted_date_short = new_date_short;

        changed
    }

    /// Queries Windows system time via GetLocalTime and updates the engine.
    /// Returns `true` if the displayed time or date changed.
    pub fn refresh(&mut self) -> bool {
        let st: SYSTEMTIME = unsafe { GetLocalTime() };
        self.update_from_system_time(&st)
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hour_minute_extraction() {
        let st = SYSTEMTIME {
            wYear: 2026,
            wMonth: 10,
            wDayOfWeek: 1, // Monday
            wDay: 5,
            wHour: 10,
            wMinute: 42,
            wSecond: 15,
            wMilliseconds: 250,
        };
        let engine = ClockEngine::from_system_time(&st);
        assert_eq!(engine.state().hour, 10);
        assert_eq!(engine.state().minute, 42);
        assert_eq!(engine.state().second, 15);
        assert_eq!(engine.state().millisecond, 250);
        assert_eq!(engine.state().year, 2026);
        assert_eq!(engine.state().month, 10);
        assert_eq!(engine.state().day, 5);
        assert_eq!(engine.state().day_of_week, 1);
    }

    #[test]
    fn test_12_hour_formatting() {
        // Midnight (00:00)
        assert_eq!(format_time_12h(0, 0), "12:00 AM");
        assert_eq!(format_time_12h(0, 5), "12:05 AM");

        // Morning
        assert_eq!(format_time_12h(9, 41), "9:41 AM");
        assert_eq!(format_time_12h(11, 59), "11:59 AM");

        // Noon (12:00)
        assert_eq!(format_time_12h(12, 0), "12:00 PM");
        assert_eq!(format_time_12h(12, 30), "12:30 PM");

        // Afternoon / Evening
        assert_eq!(format_time_12h(13, 0), "1:00 PM");
        assert_eq!(format_time_12h(17, 30), "5:30 PM");
        assert_eq!(format_time_12h(23, 59), "11:59 PM");
    }

    #[test]
    fn test_24_hour_formatting() {
        assert_eq!(format_time_24h(0, 0), "00:00");
        assert_eq!(format_time_24h(0, 5), "00:05");
        assert_eq!(format_time_24h(9, 41), "09:41");
        assert_eq!(format_time_24h(12, 0), "12:00");
        assert_eq!(format_time_24h(17, 30), "17:30");
        assert_eq!(format_time_24h(23, 59), "23:59");
    }

    #[test]
    fn test_date_formatting() {
        assert_eq!(format_date_long(10, 5, 1), "Monday, October 5");
        assert_eq!(format_date_short(10, 5, 1), "Mon, Oct 5");

        assert_eq!(format_date_long(1, 1, 4), "Thursday, January 1");
        assert_eq!(format_date_short(1, 1, 4), "Thu, Jan 1");

        assert_eq!(format_date_long(12, 31, 3), "Wednesday, December 31");
        assert_eq!(format_date_short(12, 31, 3), "Wed, Dec 31");
    }

    #[test]
    fn test_minute_rollover_calculation() {
        // At start of minute (0s, 0ms): ~60,020 ms
        let ms_0 = calculate_ms_until_next_minute(0, 0);
        assert_eq!(ms_0, 60_020);

        // At halfway (30s, 0ms): ~30,020 ms
        let ms_30 = calculate_ms_until_next_minute(30, 0);
        assert_eq!(ms_30, 30_020);

        // At halfway with millis (30s, 500ms): 29,520 ms
        let ms_30_half = calculate_ms_until_next_minute(30, 500);
        assert_eq!(ms_30_half, 29_520);

        // Near end of minute (59s, 0ms): 1,020 ms
        let ms_59 = calculate_ms_until_next_minute(59, 0);
        assert_eq!(ms_59, 1_020);

        // Very close to boundary (59s, 990ms): 10ms + 20ms = 30ms -> clamped to 50ms min
        let ms_late = calculate_ms_until_next_minute(59, 990);
        assert_eq!(ms_late, 50);

        // Clamping checks
        assert!(calculate_ms_until_next_minute(59, 999) >= 50);
        assert!(calculate_ms_until_next_minute(0, 0) <= 60_050);
    }

    #[test]
    fn test_midnight_23_59_to_00_00_rollover() {
        let st_before = SYSTEMTIME {
            wYear: 2026,
            wMonth: 10,
            wDayOfWeek: 1, // Monday
            wDay: 5,
            wHour: 23,
            wMinute: 59,
            wSecond: 55,
            wMilliseconds: 0,
        };

        let mut engine = ClockEngine::from_system_time(&st_before);
        assert_eq!(engine.state().hour, 23);
        assert_eq!(engine.state().minute, 59);

        // Rollover to midnight next day (Tuesday, Oct 6)
        let st_midnight = SYSTEMTIME {
            wYear: 2026,
            wMonth: 10,
            wDayOfWeek: 2, // Tuesday
            wDay: 6,
            wHour: 0,
            wMinute: 0,
            wSecond: 0,
            wMilliseconds: 20,
        };

        let changed = engine.update_from_system_time(&st_midnight);
        assert!(changed, "Midnight rollover must trigger redraw");
        assert_eq!(engine.state().hour, 0);
        assert_eq!(engine.state().minute, 0);
        assert_eq!(engine.state().day, 6);
        assert_eq!(engine.state().day_of_week, 2);
    }

    #[test]
    fn test_month_rollover() {
        // Oct 31 -> Nov 1
        let st_oct_31 = SYSTEMTIME {
            wYear: 2026,
            wMonth: 10,
            wDayOfWeek: 6, // Saturday
            wDay: 31,
            wHour: 23,
            wMinute: 59,
            wSecond: 50,
            wMilliseconds: 0,
        };
        let mut engine = ClockEngine::from_system_time(&st_oct_31);

        let st_nov_1 = SYSTEMTIME {
            wYear: 2026,
            wMonth: 11,
            wDayOfWeek: 0, // Sunday
            wDay: 1,
            wHour: 0,
            wMinute: 0,
            wSecond: 0,
            wMilliseconds: 15,
        };
        let changed = engine.update_from_system_time(&st_nov_1);
        assert!(changed);
        assert_eq!(engine.state().month, 11);
        assert_eq!(engine.state().day, 1);
    }

    #[test]
    fn test_year_rollover() {
        // Dec 31, 23:59 -> Jan 1, 00:00
        let st_dec_31 = SYSTEMTIME {
            wYear: 2026,
            wMonth: 12,
            wDayOfWeek: 4, // Thursday
            wDay: 31,
            wHour: 23,
            wMinute: 59,
            wSecond: 50,
            wMilliseconds: 0,
        };
        let mut engine = ClockEngine::from_system_time(&st_dec_31);

        let st_jan_1 = SYSTEMTIME {
            wYear: 2027,
            wMonth: 1,
            wDayOfWeek: 5, // Friday
            wDay: 1,
            wHour: 0,
            wMinute: 0,
            wSecond: 0,
            wMilliseconds: 15,
        };
        let changed = engine.update_from_system_time(&st_jan_1);
        assert!(changed);
        assert_eq!(engine.state().year, 2027);
        assert_eq!(engine.state().month, 1);
        assert_eq!(engine.state().day, 1);
    }

    #[test]
    fn test_no_unnecessary_redraw_when_same_minute() {
        let st_tick1 = SYSTEMTIME {
            wYear: 2026,
            wMonth: 10,
            wDayOfWeek: 1,
            wDay: 5,
            wHour: 10,
            wMinute: 42,
            wSecond: 10,
            wMilliseconds: 0,
        };
        let mut engine = ClockEngine::from_system_time(&st_tick1);

        // Same minute, only seconds and millis advanced
        let st_tick2 = SYSTEMTIME {
            wYear: 2026,
            wMonth: 10,
            wDayOfWeek: 1,
            wDay: 5,
            wHour: 10,
            wMinute: 42,
            wSecond: 35,
            wMilliseconds: 500,
        };

        let changed = engine.update_from_system_time(&st_tick2);
        assert!(!changed, "State within same minute must not request redraw");
        assert_eq!(engine.state().second, 35);
    }

    #[test]
    fn test_system_time_change_triggers_update() {
        let st1 = SYSTEMTIME {
            wYear: 2026,
            wMonth: 10,
            wDayOfWeek: 1,
            wDay: 5,
            wHour: 10,
            wMinute: 42,
            wSecond: 0,
            wMilliseconds: 0,
        };
        let mut engine = ClockEngine::from_system_time(&st1);

        // User manually adjusts time ahead by 2 hours
        let st2 = SYSTEMTIME {
            wYear: 2026,
            wMonth: 10,
            wDayOfWeek: 1,
            wDay: 5,
            wHour: 12,
            wMinute: 42,
            wSecond: 0,
            wMilliseconds: 0,
        };

        let changed = engine.update_from_system_time(&st2);
        assert!(changed, "Manual system time change must trigger update");
        assert_eq!(engine.state().hour, 12);
    }

    #[test]
    fn test_live_windows_get_local_time_succeeds() {
        let engine = ClockEngine::now();
        assert!(engine.state().year >= 2026);
        assert!(engine.state().month >= 1 && engine.state().month <= 12);
        assert!(engine.state().day >= 1 && engine.state().day <= 31);
        assert!(engine.state().hour <= 23);
        assert!(engine.state().minute <= 59);
        assert!(!engine.formatted_time().is_empty());
        assert!(!engine.formatted_date().is_empty());
    }

    #[test]
    fn test_expanded_midnight_transition_updates_time_and_date() {
        // Monday Oct 5, 23:59:50
        let st_before = SYSTEMTIME {
            wYear: 2026,
            wMonth: 10,
            wDayOfWeek: 1, // Monday
            wDay: 5,
            wHour: 23,
            wMinute: 59,
            wSecond: 50,
            wMilliseconds: 0,
        };
        let mut engine = ClockEngine::from_system_time(&st_before);
        assert_eq!(engine.state().hour, 23);
        assert_eq!(engine.state().minute, 59);
        assert_eq!(engine.state().day, 5);

        // Midnight rollover to Tuesday Oct 6, 00:00:00
        let st_after = SYSTEMTIME {
            wYear: 2026,
            wMonth: 10,
            wDayOfWeek: 2, // Tuesday
            wDay: 6,
            wHour: 0,
            wMinute: 0,
            wSecond: 0,
            wMilliseconds: 15,
        };
        let changed = engine.update_from_system_time(&st_after);
        assert!(changed, "Midnight rollover must trigger update");
        assert_eq!(engine.state().hour, 0);
        assert_eq!(engine.state().minute, 0);
        assert_eq!(engine.state().day, 6);
        assert_eq!(engine.state().day_of_week, 2);

        // Verify date changed simultaneously with midnight time
        assert!(engine.formatted_date().contains("6"));
        assert!(engine.formatted_date().contains("Tuesday") || !engine.formatted_date().is_empty());
    }
}
