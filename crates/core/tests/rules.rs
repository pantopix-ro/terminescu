//! Status precedence, expiry and deletion (task 5.3, D15): manual beats rule, rule beats auto,
//! and the needs-you marker shows over all of them.

use std::collections::HashMap;
use terminescu_core::state::{Command, Core, Workspace};
use terminescu_core::status::{
    Auto, Engine, Look, ManualStatus, PaneFacts, Rule, RuleKind, Source, Until, compute,
};
use terminescu_plugin_api::AgentState;
use terminescu_protocol::ShellState;

fn look(label: &str, accent: &str) -> Look {
    Look {
        label: label.into(),
        accent: Some(accent.into()),
        background: None,
    }
}

fn tests_red() -> Rule {
    Rule {
        id: "rule-red".into(),
        status_id: "st-red".into(),
        look: look("Tests red", "red"),
        kind: RuleKind::ExitCode {
            pattern: "pnpm test*".into(),
        },
        enabled: true,
    }
}

fn stale(minutes: u32) -> Rule {
    Rule {
        id: "rule-stale".into(),
        status_id: "st-stale".into(),
        look: look("Stale", "fg"),
        kind: RuleKind::ClaudeIdle { minutes },
        enabled: true,
    }
}

/// A run of `command` that started and ended with `code`, as the n-th run in the pane.
fn ran(n: u64, command: &str, code: i32) -> ShellState {
    ShellState {
        integrated: true,
        command: Some(command.into()),
        last_exit: Some(code),
        started: n,
        ended: n,
        ..Default::default()
    }
}

/// One window, one side tab with two tabs of one pane each: (core, side tab, tabs, panes).
fn workspace() -> (Core, String, Vec<String>, Vec<String>) {
    let mut n = 0;
    let mut core = Core::with_ids(
        Workspace::default(),
        Box::new(move || {
            n += 1;
            format!("id{n}")
        }),
    );
    let (_, panes) = core.first_start_with(2);
    let side = &core.workspace().windows[0].side_tabs[0];
    let (side_id, tabs) = (
        side.id.clone(),
        side.tabs.iter().map(|t| t.id.clone()).collect(),
    );
    (core, side_id, tabs, panes)
}

fn facts(entries: &[(&str, Option<AgentState>, Option<ShellState>)]) -> HashMap<String, PaneFacts> {
    entries
        .iter()
        .map(|(p, agent, shell)| {
            (
                p.to_string(),
                PaneFacts {
                    agent: *agent,
                    shell: shell.clone(),
                },
            )
        })
        .collect()
}

#[test]
fn manual_beats_rule_beats_auto_and_waiting_shows_over_all() {
    let (mut core, side, tabs, panes) = workspace();
    let mut engine = Engine::default();
    engine.set_rules(vec![tests_red()]);
    let failed = ran(1, "pnpm test --filter api", 1);
    assert!(engine.shell(&panes[0], None, &failed));

    let f = facts(&[
        (&panes[0], None, Some(failed.clone())),
        (&panes[1], Some(AgentState::Running), None),
    ]);
    let map = compute(core.workspace(), &f, &engine);
    assert_eq!(map[&tabs[0]].source, Source::Rule);
    assert_eq!(map[&tabs[0]].auto, Auto::Failed);
    assert_eq!(map[&tabs[1]].source, Source::Auto);
    assert_eq!(map[&tabs[1]].auto, Auto::Running);
    // The side tab carries its panes' rule status, over its most urgent auto status.
    assert_eq!(map[&side].source, Source::Rule);
    assert_eq!(map[&side].look.as_ref().unwrap().label, "Tests red");
    assert_eq!(map[&side].auto, Auto::Running);

    core.handle(Command::SetStatus {
        target_id: side.clone(),
        status: Some(ManualStatus {
            status_id: None,
            look: look("Blocked", "magenta"),
        }),
    })
    .unwrap();
    let waiting = facts(&[
        (&panes[0], None, Some(failed)),
        (&panes[1], Some(AgentState::Waiting), None),
    ]);
    let map = compute(core.workspace(), &waiting, &engine);
    assert_eq!(map[&side].source, Source::Manual);
    assert_eq!(map[&side].look.as_ref().unwrap().label, "Blocked");
    assert!(map[&side].waiting, "amber shows over a manual status");
    assert_eq!(map[&side].auto, Auto::Waiting);
    assert_eq!(
        map[&tabs[0]].source,
        Source::Rule,
        "a tab keeps its own status"
    );
}

#[test]
fn rule_status_holds_until_the_command_runs_again() {
    let (_, _, _, panes) = workspace();
    let p = &panes[0];
    let mut engine = Engine::default();
    engine.set_rules(vec![tests_red()]);
    let first = ran(1, "pnpm test", 1);
    engine.shell(p, None, &first);
    assert_eq!(engine.dynamic(p).len(), 1);

    // Another command leaves it; the next `pnpm test` start clears it.
    let ls = ran(2, "ls", 0);
    assert!(!engine.shell(p, Some(&first), &ls));
    assert_eq!(engine.dynamic(p).len(), 1);
    let rerun = ShellState {
        running: true,
        started: 3,
        ..ran(2, "pnpm test", 0)
    };
    assert!(engine.shell(p, Some(&ls), &rerun));
    assert!(engine.dynamic(p).is_empty());

    // A run that started and failed between two reads still sets it.
    let fast = ran(4, "pnpm test", 2);
    assert!(engine.shell(p, Some(&rerun), &fast));
    assert_eq!(engine.dynamic(p)[0].look.label, "Tests red");
}

#[test]
fn cli_statuses_expire_by_time_and_at_command_end() {
    let (_, _, _, panes) = workspace();
    let p = &panes[0];
    let mut engine = Engine::default();
    let none: HashMap<String, Option<i64>> = HashMap::new();
    engine.cli(p, Some(look("Migrating", "cyan")), Until::At(1_000));
    assert!(!engine.tick(999, &none));
    assert!(engine.tick(1_000, &none));
    assert!(engine.dynamic(p).is_empty());

    let running = ShellState {
        running: true,
        started: 1,
        command: Some("./deploy.sh".into()),
        ..Default::default()
    };
    engine.shell(p, None, &running);
    engine.cli(p, Some(look("Deploying", "cyan")), Until::CommandEnd);
    let done = ShellState {
        running: false,
        ended: 1,
        last_exit: Some(0),
        ..running.clone()
    };
    assert!(engine.shell(p, Some(&running), &done));
    assert!(engine.dynamic(p).is_empty());

    engine.cli(p, Some(look("Review", "blue")), Until::Never);
    engine.cli(p, None, Until::Never);
    assert!(engine.dynamic(p).is_empty(), "--clear drops it");
}

#[test]
fn claude_idle_rule_fires_after_the_wait_and_clears_on_activity() {
    let (_, _, _, panes) = workspace();
    let p = panes[0].clone();
    let mut engine = Engine::default();
    engine.set_rules(vec![stale(30)]);
    let idle = |since: Option<i64>| HashMap::from([(p.clone(), since)]);
    assert!(!engine.tick(29 * 60_000, &idle(Some(0))));
    assert!(engine.tick(30 * 60_000, &idle(Some(0))));
    assert_eq!(engine.dynamic(&p)[0].look.label, "Stale");
    assert!(!engine.tick(31 * 60_000, &idle(Some(0))), "set once");
    assert!(engine.tick(32 * 60_000, &idle(None)));
    assert!(engine.dynamic(&p).is_empty());
}

#[test]
fn deleting_a_status_in_use_sends_its_tabs_back_to_auto() {
    let (mut core, side, tabs, panes) = workspace();
    let review = ManualStatus {
        status_id: Some("st-review".into()),
        look: look("Review", "cyan"),
    };
    for target in [&side, &tabs[1]] {
        core.handle(Command::SetStatus {
            target_id: target.clone(),
            status: Some(review.clone()),
        })
        .unwrap();
    }
    core.handle(Command::SetStatus {
        target_id: tabs[0].clone(),
        status: Some(ManualStatus {
            status_id: None,
            look: look("Mine", "green"),
        }),
    })
    .unwrap();

    // Editing the saved status restyles every user of it.
    let b = core
        .handle(Command::RestyleStatus {
            status_id: "st-review".into(),
            status: Some(ManualStatus {
                status_id: None,
                look: look("In review", "blue"),
            }),
        })
        .unwrap();
    assert_eq!(b.patches.len(), 2);
    let ws = core.workspace();
    assert_eq!(
        ws.side_tab(&side)
            .unwrap()
            .status
            .as_ref()
            .unwrap()
            .look
            .label,
        "In review"
    );

    core.handle(Command::RestyleStatus {
        status_id: "st-review".into(),
        status: None,
    })
    .unwrap();
    let mut engine = Engine::default();
    let map = compute(
        core.workspace(),
        &facts(&[(&panes[0], None, None)]),
        &engine,
    );
    assert_eq!(map[&side].source, Source::Auto);
    assert_eq!(map[&tabs[1]].source, Source::Auto);
    assert_eq!(
        map[&tabs[0]].source,
        Source::Manual,
        "a status of its own stays"
    );

    // A deleted rule takes the status it set with it.
    engine.set_rules(vec![tests_red()]);
    engine.shell(&panes[0], None, &ran(1, "pnpm test", 1));
    engine.set_rules(vec![]);
    assert!(engine.dynamic(&panes[0]).is_empty());
}

#[test]
fn a_status_with_amber_or_an_unknown_target_is_refused() {
    let (mut core, side, _, _) = workspace();
    assert!(
        core.handle(Command::SetStatus {
            target_id: side,
            status: Some(ManualStatus {
                status_id: None,
                look: look("Busy", "amber"),
            }),
        })
        .is_err()
    );
    assert!(
        core.handle(Command::SetStatus {
            target_id: "nope".into(),
            status: None,
        })
        .is_err()
    );
}

#[test]
fn a_new_shell_drops_statuses_tied_to_the_old_one() {
    let (_, _, _, panes) = workspace();
    let p = &panes[0];
    let mut engine = Engine::default();
    engine.set_rules(vec![tests_red()]);
    engine.shell(p, None, &ran(5, "pnpm test", 1));
    engine.cli(p, Some(look("Migrating", "cyan")), Until::At(i64::MAX));
    assert!(engine.shell_restarted(p));
    let left: Vec<_> = engine
        .dynamic(p)
        .iter()
        .map(|d| d.look.label.as_str())
        .collect();
    assert_eq!(left, ["Migrating"], "a --for status outlives the shell");
    // The new shell counts from zero, and its first failing run fires the rule again.
    assert!(engine.shell(p, None, &ran(1, "pnpm test", 1)));
}
