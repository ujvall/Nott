//! Spaces: what the expanded notch presents. Phase 4.1 is state only: every
//! space renders the existing UI. The active space lives in `WindowState` and is
//! never persisted (each launch starts in `Home`). All spaces share the one
//! `MediaEngine` (session, metadata, artwork, accent, timeline, visualizer).

/// The active space. Adding a variant is a compile error at every exhaustive
/// `match`, so an unknown space cannot appear by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NottSpace {
    #[default]
    Home,
    Music,
    /// The in-memory clipboard history (Phase 5.2).
    Clipboard,
}

/// What the expanded notch shows: a space, or the drop page while an image is
/// dragged over it. Animations blend between two scenes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    Space(NottSpace),
    Drop,
}

impl NottSpace {
    #[allow(dead_code)]
    pub const ALL: [Self; 3] = [Self::Home, Self::Music, Self::Clipboard];

    #[allow(dead_code)]
    pub fn is_home(self) -> bool {
        self == Self::Home
    }

    #[allow(dead_code)]
    pub fn is_music(self) -> bool {
        self == Self::Music
    }

    /// Selector label / accessible name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Music => "Music",
            Self::Clipboard => "Clipboard",
        }
    }

    /// Switches to `space`; returns true if the active space changed. The single
    /// transition point (the space selector calls this via `WindowState`).
    pub fn switch_to(&mut self, space: Self) -> bool {
        std::mem::replace(self, space) != space
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::{MediaContent, PlaybackState, Visualizer};

    #[test]
    fn test_default_is_home_and_spaces_are_distinct() {
        let s = NottSpace::default();
        assert_eq!(s, NottSpace::Home);
        assert!(s.is_home() && !s.is_music());
        assert_ne!(NottSpace::Home, NottSpace::Music);
        assert!(NottSpace::Music.is_music() && !NottSpace::Music.is_home());
    }

    #[test]
    fn test_switching_home_music_and_back() {
        let mut s = NottSpace::default();
        assert!(s.switch_to(NottSpace::Music), "Home -> Music");
        assert!(s.is_music());
        assert!(!s.switch_to(NottSpace::Music), "same space is not a change");
        assert!(s.switch_to(NottSpace::Home), "Music -> Home");
        assert!(s.is_home());
        for i in 0..100 {
            let target = NottSpace::ALL[i % 3];
            s.switch_to(target);
            assert_eq!(s, target, "repeated switching");
        }
    }

    #[test]
    fn test_labels() {
        assert_eq!(
            NottSpace::ALL.map(NottSpace::label),
            ["Home", "Music", "Clipboard"]
        );
    }

    #[test]
    fn test_exactly_three_spaces() {
        // Exhaustive match (no wildcard): a fourth variant fails to compile here
        let index = |s: NottSpace| match s {
            NottSpace::Home => 0,
            NottSpace::Music => 1,
            NottSpace::Clipboard => 2,
        };
        assert_eq!(NottSpace::ALL.len(), 3);
        assert_eq!(NottSpace::ALL.map(index), [0, 1, 2]);
    }

    #[test]
    fn test_home_clipboard_music_round_trip() {
        let mut s = NottSpace::default();
        assert!(s.switch_to(NottSpace::Clipboard));
        assert!(s.switch_to(NottSpace::Music));
        assert!(s.switch_to(NottSpace::Clipboard));
        assert!(s.switch_to(NottSpace::Home));
        assert!(s.is_home());
    }

    #[test]
    fn test_switching_leaves_shared_media_state_alone() {
        // Spaces hold no media data: the shared model is identical before/after
        let content = MediaContent {
            title: "Song".into(),
            playback: PlaybackState::Playing,
            accent: Some([10, 20, 30]),
            ..MediaContent::test_default()
        };
        let mut viz = Visualizer::default();
        viz.set_playing(true);
        viz.step(100.0);
        let (before_content, before_viz) = (content.clone(), viz);
        let mut s = NottSpace::default();
        s.switch_to(NottSpace::Music);
        s.switch_to(NottSpace::Home);
        s.switch_to(NottSpace::Music);
        assert_eq!(content, before_content);
        assert_eq!(viz, before_viz);
        // ...and media updates don't touch the space
        let mut content = content;
        content.title = "Next".into();
        content.playback = PlaybackState::Paused;
        assert!(s.is_music(), "space survives media updates");
    }

    #[test]
    fn test_no_persistence_each_start_is_home() {
        // A fresh state (as at every launch) is always Home, whatever was active
        let mut previous = NottSpace::default();
        previous.switch_to(NottSpace::Music);
        assert_eq!(NottSpace::default(), NottSpace::Home);
    }
}
