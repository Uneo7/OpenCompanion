use crate::app::actions::Action;
use crate::app::state::EqState;
use crate::assets::icons;
use crate::i18n::{t, t_with, tr};
use crate::model::{Band, BandType};
use crate::theme;
use crate::views::actions::ViewAction;
use crate::views::Cx;
use crate::widgets::fmt::tr_type;
use crate::widgets::menu::{Entry, Lead, Meta};

pub fn profile_menu(cx: &Cx, active: usize) -> Vec<Entry<Action>> {
    let mut e = vec![Entry::Group(tr("profile.menu.group").into())];
    for (i, p) in cx.device.profiles().iter().enumerate() {
        let item = Entry::plain(format!("{}  {}", i + 1, cx.profile_name(i)), Action::ActivateProfile(i)).lead(Lead::Mark(theme::rgb(p.hue().a)));
        e.push(if i == active { item.meta(Meta::Check) } else { item });
    }
    e
}

pub fn edit_menu(cx: &Cx, active: usize) -> Vec<Entry<Action>> {
    let mark = theme::rgb(cx.device.profiles()[active].hue().a);
    let mut e = vec![
        Entry::Group(cx.profile_name(active)),
        Entry::item(tr("profile.menu.rename"), icons::PENCIL, Action::AskRename(active)),
        Entry::plain(tr("profile.menu.color"), Action::AskHue(active)).lead(Lead::Mark(mark)),
    ];
    e.push(Entry::Sep);
    e.extend(duplicate_entries(cx, active));
    e.extend([
        Entry::Sep,
        Entry::item(tr("profile.export"), icons::EXPORT, Action::View(ViewAction::AskExport(active))),
        Entry::item(tr("profile.import"), icons::IMPORT, Action::View(ViewAction::AskImport(active))),
        Entry::Sep,
        Entry::item(tr("profile.flat"), icons::ROTATE, Action::AskFlatten(active)),
    ]);
    e
}

/// "Dupliquer dans…": always the choice of the target, among the other profiles (never the source, never the
/// locked "Bose® d'origine"), each with its name and colour; every choice asks before replacing.
pub fn duplicate_entries(cx: &Cx, from: usize) -> Vec<Entry<Action>> {
    let profiles = cx.device.profiles();
    let mut e = vec![Entry::Group(tr("profile.duplicate.into").into())];
    for (i, p) in profiles.iter().enumerate().filter(|(i, p)| *i != from && !p.locked) {
        let label = format!("{}  {}", i + 1, cx.profile_name(i));
        e.push(Entry::plain(label, Action::AskDuplicate { from, to: i }).lead(Lead::Mark(theme::rgb(p.hue().a))));
    }
    e
}

pub fn more_menu(eq: &EqState) -> Vec<Entry<Action>> {
    vec![
        Entry::Group(tr("eq.more.g.listen").into()),
        Entry::item(tr("eq.more.ab"), icons::EAR, Action::ToggleCompare).meta(Meta::Toggle(eq.compare)),
        Entry::item(tr("eq.more.bypass"), icons::BYPASS, Action::ToggleBypass).meta(Meta::Toggle(eq.bypass)),
        Entry::Group(tr("eq.more.g.view").into()),
        Entry::item(tr("eq.more.live"), icons::PULSE, Action::ToggleLive).meta(Meta::Toggle(eq.live)),
        Entry::Sep,
        Entry::item(tr("eq.more.reset"), icons::ROTATE, Action::AskResetTarget),
    ]
}

pub fn type_menu(i: usize, b: &Band) -> Vec<Entry<Action>> {
    BandType::ALL
        .iter()
        .map(|kind| {
            let item = Entry::plain(tr_type(*kind), Action::SetBandType(i, *kind));
            if *kind == b.kind { item.meta(Meta::Check) } else { item }
        })
        .collect()
}

pub fn band_menu(i: usize, b: &Band) -> Vec<Entry<Action>> {
    let mut e = vec![Entry::Group(t_with("eq.band", &[("n", &(i + 1))]))];
    for kind in BandType::ALL {
        let item = Entry::item(tr_type(kind), icons::WAVE, Action::SetBandType(i, kind));
        e.push(if kind == b.kind { item.meta(Meta::Check) } else { item });
    }
    e.push(Entry::Sep);
    e.push(Entry::item(tr("eq.menu.gain0"), icons::ROTATE, Action::ZeroGain(i)));
    e.push(Entry::item(tr("eq.menu.active"), icons::BYPASS, Action::ToggleBand(i)).meta(Meta::Toggle(b.on)));
    e.push(Entry::item(tr("eq.menu.delete"), icons::TRASH, Action::DeleteBand).meta(Meta::Kbd(tr("kbd.delete").into())).danger());
    e
}

const GESTURES: usize = 9;

pub fn help() -> Vec<Entry<Action>> {
    let rows = (1..=GESTURES).map(|i| (t(&format!("eq.help.g{i}")), t(&format!("eq.help.e{i}")))).collect();
    vec![Entry::Group(tr("eq.help.title").into()), Entry::Pairs(rows)]
}
