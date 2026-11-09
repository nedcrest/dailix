//! The pane's Voice row (Omnix gap 77): the clip's sound as filmed and its
//! cleaned voices (`dailix_sheet::voice`), each heard at one loudness so a
//! louder one doesn't pass for better, the one the edit uses chosen (the
//! marking's `audio`), a missing one made from the original off the UI
//! thread.

use super::pane::PaneMsg;
use super::{App, Message, tip};
use dailix_sheet::voice;
use dailix_sheet::{ClipOp, Op};
use iced::widget::{button, column, pick_list, row, text, tooltip};
use iced::{Alignment, Element, Task};

/// The clip's voices as the pane knows them.
#[derive(Default)]
pub struct Voices {
    /// The made ones: their recipe's name and loudness.
    pub made: Vec<(String, Option<f64>)>,
    /// The clip's own sound's loudness, once measured.
    pub own: Option<f64>,
    /// The one heard; none: the clip's own.
    pub heard: Option<String>,
    /// The recipe picked to make, and the one being made.
    pub recipe: Option<String>,
    pub making: Option<String>,
    /// Where it holds speech, seconds (gap 83), found in a cleaned voice.
    pub speech: Vec<(f64, f64)>,
}

/// What reading a clip's voices found.
#[derive(Debug, Clone, Default)]
pub struct Listed {
    made: Vec<(String, Option<f64>)>,
    own: Option<f64>,
    speech: Vec<(f64, f64)>,
}

#[derive(Debug, Clone)]
pub enum VoiceMsg {
    /// Clip `.0`'s voices read, its own sound measured, its speech found.
    Listed(String, Listed),
    Hear(Option<String>),
    /// The one heard is the one the edit uses.
    Use,
    Recipe(String),
    Make,
    Made(String, Result<(), String>),
}

impl App {
    /// Reads clip `key`'s voices, measures its own sound and finds its
    /// speech, off the UI thread (a few seconds' decoding for a long clip).
    pub(super) fn list_voices(&self, key: &str) -> Task<Message> {
        let Some(p) = &self.project else { return Task::none() };
        let (layout, key) = (p.footage.layout.clone(), key.to_owned());
        let chosen = self.chosen_voice();
        Task::perform(
            super::blocking(move || {
                let listed = read_voices(&layout, &key, chosen.as_deref());
                (key, listed)
            }),
            |(key, listed)| Message::Pane(PaneMsg::Voice(VoiceMsg::Listed(key, listed))),
        )
    }

    /// The voice the edit uses, by the marking.
    fn chosen_voice(&self) -> Option<String> {
        let (p, key) = (self.project.as_ref()?, self.pane.key.as_ref()?);
        p.sheet.clips.get(key)?.audio.clone()
    }

    /// The player plays the heard voice, at `voice::HEARD_AT`.
    pub(super) fn play_heard(&mut self) {
        let Some(p) = &self.project else { return };
        let Some(key) = &self.pane.key else { return };
        let v = &self.pane.voices;
        let (file, lufs) = match &v.heard {
            Some(name) => (voice::dir(&p.footage.layout, key).map(|d| voice::track(&d, name)), v.made.iter().find(|(n, _)| n == name).and_then(|(_, l)| *l)),
            None => (None, v.own),
        };
        let gain = voice::compare_gain(lufs);
        if let Some(viewer) = &mut self.pane.viewer {
            viewer.hear(file.as_deref(), gain);
        }
    }

    pub(super) fn on_voice(&mut self, msg: VoiceMsg) -> Task<Message> {
        match msg {
            VoiceMsg::Listed(key, listed) => {
                if self.pane.key.as_deref() == Some(key.as_str()) {
                    let made = listed.made;
                    let chosen = self.chosen_voice().filter(|c| made.iter().any(|(n, _)| n == c));
                    let v = &mut self.pane.voices;
                    // Heard as the edit will have it, unless one was picked.
                    v.heard = v.heard.take().filter(|h| made.iter().any(|(n, _)| n == h)).or(chosen);
                    (v.made, v.own, v.speech) = (made, listed.own, listed.speech);
                    self.play_heard();
                }
            }
            VoiceMsg::Hear(heard) => {
                self.pane.voices.heard = heard;
                self.play_heard();
            }
            VoiceMsg::Use => {
                if let Some(clip) = self.pane.key.clone() {
                    self.change(Op::Clip(ClipOp::Voice { clip: clip.clone(), audio: self.pane.voices.heard.clone() }));
                    // Speech is looked for in the chosen voice first.
                    return self.list_voices(&clip);
                }
            }
            VoiceMsg::Recipe(name) => self.pane.voices.recipe = Some(name),
            VoiceMsg::Make => return self.make_voice(),
            VoiceMsg::Made(key, done) => {
                self.pane.voices.making = None;
                if let Err(e) = done {
                    self.error = Some(e);
                } else if self.pane.key.as_deref() == Some(key.as_str()) {
                    return self.list_voices(&key);
                }
            }
        }
        Task::none()
    }

    /// The recipe the Make button makes: the one picked, else the first
    /// not made yet.
    fn recipe_to_make(&self) -> Option<&'static voice::Recipe> {
        let v = &self.pane.voices;
        let picked = v.recipe.as_deref().and_then(voice::recipe);
        picked.or_else(|| voice::RECIPES.iter().find(|r| !v.made.iter().any(|(n, _)| n == r.name)))
    }

    fn make_voice(&mut self) -> Task<Message> {
        let (Some(p), Some(key), Some(recipe)) = (&self.project, self.pane.key.clone(), self.recipe_to_make()) else { return Task::none() };
        if self.pane.voices.making.is_some() {
            return Task::none();
        }
        let layout = p.footage.layout.clone();
        let source = layout.clip(&key);
        self.pane.voices.making = Some(recipe.name.to_owned());
        Task::perform(super::blocking(move || (key.clone(), crate::voice::make(&layout, &key, &source, recipe).map(|_| ()))), |(key, done)| Message::Pane(PaneMsg::Voice(VoiceMsg::Made(key, done))))
    }

    /// The row: the clip's own sound and each voice made (heard lit, the
    /// edit's ✓), «Use for the edit», and a recipe to make.
    pub(super) fn voice_row(&self) -> Element<'_, Message> {
        let to = |m: VoiceMsg| Message::Pane(PaneMsg::Voice(m));
        let v = &self.pane.voices;
        let chosen = self.chosen_voice();
        let lufs = |l: Option<f64>| l.map_or(String::new(), |l| format!(" {l:.0}"));
        let tile = |name: Option<&str>, l: Option<f64>| {
            let heard = v.heard.as_deref() == name;
            let tick = if chosen.as_deref() == name { "✓ " } else { "" };
            let label = format!("{tick}{}{}", name.unwrap_or("Original"), lufs(l));
            button(text(label).size(12)).padding([2, 8]).style(if heard { button::primary } else { button::secondary }).on_press(to(VoiceMsg::Hear(name.map(str::to_owned)))).into()
        };
        let tiles: Vec<Element<'_, Message>> = std::iter::once(tile(None, v.own)).chain(v.made.iter().map(|(n, l)| tile(Some(n), *l))).collect();
        let tiles = tip(row(tiles).spacing(4).wrap(), text(format!("Hear each at {:.0} LUFS (numbers: as made); ✓ the one the edit uses", voice::HEARD_AT)), tooltip::Position::Top);
        let use_it = button(text("Use for the edit").size(12)).padding([2, 8]).style(button::secondary).on_press_maybe((v.heard != chosen).then(|| to(VoiceMsg::Use)));
        let names: Vec<String> = voice::RECIPES.iter().map(|r| r.name.to_owned()).collect();
        let next = self.recipe_to_make().map(|r| r.name.to_owned());
        let pick = pick_list(names, next.clone(), move |n| to(VoiceMsg::Recipe(n))).text_size(12).padding([2, 6]);
        let make = button(text(v.making.as_ref().map_or("Make".to_owned(), |m| format!("Making {m}…"))).size(12)).padding([2, 8]).on_press_maybe((v.making.is_none() && next.is_some()).then(|| to(VoiceMsg::Make)));
        let about = next.as_deref().and_then(voice::recipe).map_or("", |r| r.note);
        let make = tip(row![pick, make].spacing(4).align_y(Alignment::Center), text(format!("Made again from the original: {about}")), tooltip::Position::Top);
        column![row![text("Voice").size(13), tiles].spacing(8).align_y(Alignment::Center), row![use_it, make].spacing(8).align_y(Alignment::Center)].spacing(4).into()
    }
}

/// Clip `key`'s voices: each made one and its loudness, its own sound's
/// loudness, and its speech, found in a cleaned voice
/// (`voice::speech_source`; none without one). Blocking.
fn read_voices(layout: &dailix_sheet::Layout, key: &str, chosen: Option<&str>) -> Listed {
    let own = omnix::sound::loudness(&layout.clip(key)).ok().flatten();
    let Some(dir) = voice::dir(layout, key) else { return Listed { own, ..Listed::default() } };
    let made = voice::made(&dir);
    let names = voice::variants(&dir);
    let speech = voice::speech_source(&names, chosen).and_then(|n| omnix::sound::speech(&voice::track(&dir, n)).ok()).unwrap_or_default();
    Listed { made: names.iter().map(|n| (n.clone(), made.get(n).and_then(|m| m.lufs))).collect(), own, speech }
}
