//! `planroom focus --picker lens|project|stage`: the question the pane asks as a card (DR-77).
//!
//! A [`Picker`] holds up to [`PICKER_ENTRIES`] entries, each a label, a detail and the exact
//! handle picking it runs. It is built from one snapshot, folded with the workspace's recorded
//! pack set (DR-114), so the lenses, the levels and the stages are the ones the pane reads. The
//! entry whose handle is the pane's next action comes first. A picker holds no repo text but the
//! entry labels, which are masked like other names, and the stage picker states what a pick costs
//! (`estimate`) before anything is spent.

use std::path::Path;

use planroom_contract::{
    CommandName, FocusReq, FocusResult, Handle, PickEstimate, Picker, PickerEntry, PickerName,
};
use planroom_kernel::fold::Folded;
use planroom_kernel::ladder::Ladder;
use planroom_kernel::plan::{self, Project};
use planroom_store::Store;

use crate::app::error::AppError;
use crate::app::{Reply, draft, focus, intent, map};
use crate::packs::{Loaded, state};
use crate::{live, text};

/// Entries a picker holds at most: one for each key the pane gives a picker, `@planroom/ui`'s
/// `CARD_KEYS` and then its `ROW_KEYS`. The rest are counted in `more`.
pub const PICKER_ENTRIES: usize = 16;
/// The longest label an entry carries, so a card's row stays short.
const LABEL_CHARS: usize = 20;

/// The picker `name` asks, over one snapshot of `store`.
pub(crate) fn run(
    req: &FocusReq,
    name: PickerName,
    store: &Store,
    root: &Path,
) -> Result<Reply<Picker>, AppError> {
    let snapshot = store.snapshot()?;
    let set = state::for_snapshot(&snapshot, root)?;
    let vocabulary = set.vocabulary();
    let folded = live::folded(&snapshot, vocabulary)?;
    let next = focus::next_in(&snapshot, vocabulary)?;
    let mut warnings = Vec::new();
    set.warn(&mut warnings);

    let mut picker = Picker {
        name,
        question: String::new(),
        why: String::new(),
        entries: vec![],
        more: 0,
        estimate: None,
    };
    let projects = plan::projects(&folded);
    match name {
        PickerName::Lens => {
            picker.question = text::get("picker.lens_question", &[]);
            picker.why = text::get("picker.lens_why", &[]);
            picker.entries = lenses(vocabulary);
        }
        PickerName::Project => {
            picker.question = text::get("picker.project_question", &[]);
            picker.why = text::get("picker.project_why", &[]);
            let current = if req.project.is_some() {
                intent::current(&projects, req.project.as_deref())?.map(|p| p.id.clone())
            } else {
                projects.first().filter(|_| projects.len() == 1).map(|p| p.id.clone())
            };
            if projects.is_empty() {
                picker.why = intent::no_intent().line.unwrap_or_default();
            }
            picker.entries =
                projects.iter().map(|p| project_entry(&folded, p, current.as_ref() == Some(&p.id))).collect();
        }
        PickerName::Stage => {
            picker.estimate = Some(draft::estimate());
            picker.question = stage_question(&draft::estimate());
            picker.why = text::get("picker.stage_why", &[]);
            if projects.len() > 1 && req.project.is_none() {
                picker.why = text::get("picker.pick_project", &[]);
            } else if let Some(project) = intent::current(&projects, req.project.as_deref())? {
                picker.entries = stages(&set.loaded, &folded, project, req.project.is_some());
            } else {
                picker.why = intent::no_intent().line.unwrap_or_default();
            }
        }
    }
    // The entry that is the pane's next action comes first.
    if let Some(at) = picker.entries.iter().position(|e| same(&e.handle, &next)) {
        picker.entries[..=at].rotate_right(1);
    }
    if picker.entries.len() > PICKER_ENTRIES {
        picker.more = (picker.entries.len() - PICKER_ENTRIES) as u32;
        picker.entries.truncate(PICKER_ENTRIES);
    }
    let text = std::iter::once(picker.question.clone())
        .chain(
            picker
                .entries
                .iter()
                .map(|e| text::get("picker.entry", &[("label", &e.label), ("detail", &e.detail)])),
        )
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Reply { text, result: picker, seq: snapshot.seq(), warnings })
}

/// Whether two handles run the same command.
fn same(a: &Handle, b: &Handle) -> bool {
    a.command == b.command && a.args == b.args
}

/// The stage picker's question: what a pick costs at most.
fn stage_question(estimate: &PickEstimate) -> String {
    text::get(
        "picker.stage_question",
        &[("tokens", &thousands(estimate.tokens)), ("usd", &format!("{:.2}", estimate.usd))],
    )
}

/// `n` with a comma every three digits.
fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Every loaded lens, the default first, then `all`.
fn lenses(vocabulary: &planroom_contract::Vocabulary) -> Vec<PickerEntry> {
    let ladder = Ladder::of(vocabulary);
    let default = map::lens::default_id(vocabulary);
    let handle = |id: &str| Handle {
        command: CommandName("map".into()),
        args: vec!["--lens".into(), id.to_owned()],
        label: text::label("picker.lens_pick_label", &[]),
    };
    let mut out: Vec<PickerEntry> = vocabulary
        .lenses
        .iter()
        .map(|lens| PickerEntry {
            label: text::short(&lens.label, LABEL_CHARS),
            detail: text::get("picker.lens_detail", &[("level", &map::label_of(&ladder, &lens.level))]),
            handle: handle(&lens.id),
            current: lens.id == default,
        })
        .collect();
    if let Some(at) = vocabulary.lenses.iter().position(|l| l.id == default) {
        out[..=at].rotate_right(1);
    }
    out.push(PickerEntry {
        label: text::label("picker.lens_all_label", &[]),
        detail: text::get("picker.lens_all_detail", &[]),
        handle: handle(map::lens::ALL),
        current: default == map::lens::ALL,
    });
    out
}

/// A plan project: its name, its items and the ghosts it waits on.
fn project_entry(folded: &Folded, project: &Project, current: bool) -> PickerEntry {
    let items = plan::items_of(folded, &project.id);
    let waiting = items.iter().filter(|i| !i.live).count() + usize::from(!project.live);
    PickerEntry {
        label: text::short(&project.name, LABEL_CHARS),
        detail: text::get("picker.project_detail", &[("items", &items.len()), ("ghosts", &waiting)]),
        handle: Handle {
            command: CommandName("intent list".into()),
            args: vec!["--project".into(), project.id.0.clone()],
            label: text::label("picker.project_pick_label", &[]),
        },
        current,
    }
}

/// The stages of `project`, in the pack's order with the next one first. A stage's label is its
/// own, else its ID; `named` adds the project to each handle.
fn stages(loaded: &Loaded, folded: &Folded, project: &Project, named: bool) -> Vec<PickerEntry> {
    let declared = intent::stages(loaded);
    let items = plan::items_of(folded, &project.id);
    let next = plan::next_stage(&declared, &items).map(|s| s.id.clone());
    let mut out: Vec<PickerEntry> = declared
        .iter()
        .map(|stage| {
            let made = items.iter().filter(|i| stage.items.iter().any(|t| t.id == i.item_type)).count();
            let found = intent::findings(loaded, folded, &project.id, |r| stage.lint.contains(&r.id)).len();
            let detail = if next.as_deref() == Some(stage.id.as_str()) {
                text::label("picker.stage_detail_next", &[("count", &made)])
            } else {
                text::get("picker.stage_detail", &[("items", &made), ("findings", &found)])
            };
            let project_args = |mut args: Vec<String>| {
                if named {
                    args.extend(["--project".into(), project.id.0.clone()]);
                }
                args
            };
            let handle = if stage.items.is_empty() {
                Handle {
                    command: CommandName("intent lint".into()),
                    args: project_args(vec![]),
                    label: text::label("picker.lint_pick_label", &[]),
                }
            } else {
                Handle {
                    command: CommandName("intent next".into()),
                    args: project_args(vec!["--stage".into(), stage.id.clone()]),
                    label: text::label("picker.stage_pick_label", &[]),
                }
            };
            PickerEntry {
                label: text::short(stage.label.as_deref().unwrap_or(&stage.id), LABEL_CHARS),
                detail,
                handle,
                current: false,
            }
        })
        .collect();
    if let Some(at) = declared.iter().position(|s| Some(&s.id) == next.as_ref()) {
        out[..=at].rotate_right(1);
    }
    out
}

/// Fit `result` to the reply's budget by one step: for a pane, the mini-map's last block; for a
/// picker, its last entry, counted in `more`. False when there is nothing left to drop.
pub fn fit(result: &mut FocusResult) -> bool {
    match result {
        FocusResult::Pane(pane) => focus::drop_block(pane),
        FocusResult::Picker(picker) => {
            if picker.entries.pop().is_none() {
                return false;
            }
            picker.more += 1;
            true
        }
    }
}
