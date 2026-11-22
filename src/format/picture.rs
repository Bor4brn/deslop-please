//! The picture under each report, in box-drawing characters. What the report
//! is about is drawn heavy, and in red when there is color, so it reads the
//! same without.

use super::Paint;
use crate::lint::{HELD_IN_MIND, Picture, Step};

/// The longest bar, in cells: with a list's name and length, a line fits in
/// 80 columns. A longer list's bar ends in `⋯`.
const BAR: usize = 40;

/// The lines of `picture`, indented to sit under a report's notes.
pub fn draw(picture: &Picture, paint: &Paint) -> Vec<String> {
    let lines = match picture {
        Picture::Circle { steps, rest } => circle(steps, rest.as_deref(), paint),
        Picture::Spread {
            owner,
            field,
            modules,
            others,
        } => spread(owner, field, modules, others, paint),
        Picture::Lists {
            shown,
            hidden,
            lists,
        } => bars(*shown, *hidden, lists, paint),
    };
    lines
        .into_iter()
        .map(|line| format!("    {line}"))
        .collect()
}

/// Each member under the last, the tie to the next between them, and the
/// last tie back up to the first. The first tie, the cheapest, is heavy.
fn circle(steps: &[Step], rest: Option<&str>, paint: &Paint) -> Vec<String> {
    let sites = short_sites(steps);
    let width = sites.iter().map(String::len).max().unwrap_or(0);
    let mut lines = Vec::new();
    for (index, (step, site)) in steps.iter().zip(&sites).enumerate() {
        let cheapest = index == 0;
        let heavy = |light: &str, heavy: &str| {
            if cheapest {
                paint.error(heavy)
            } else {
                light.to_owned()
            }
        };
        let into = if cheapest { "┌─▶ " } else { "│   " };
        lines.push(format!("{into}{}", paint.bold(&step.member)));
        lines.push(format!(
            "│     {} {site:<width$}  {}",
            heavy("│", "┃"),
            step.uses
        ));
        if index + 1 < steps.len() {
            lines.push(format!("│     {}", heavy("▼", "▼")));
        }
    }
    lines.push("└─────┘".to_owned());
    lines.extend(rest.map(str::to_owned));
    lines
}

/// `kinds.rs:10` for `src/kinds.rs:10`: each step's site, from the directory
/// all of them are in.
fn short_sites(steps: &[Step]) -> Vec<String> {
    fn directory(step: &Step) -> Vec<&str> {
        let mut parts: Vec<&str> = step.site.file.split('/').collect();
        parts.pop();
        parts
    }
    let mut common = steps.first().map(directory).unwrap_or_default();
    for step in steps {
        let same = common
            .iter()
            .zip(directory(step))
            .take_while(|(a, b)| **a == *b)
            .count();
        common.truncate(same);
    }
    let skip: usize = common.iter().map(|part| part.len() + 1).sum();
    steps
        .iter()
        .map(|step| format!("{}:{}", &step.site.file[skip..], step.site.line))
        .collect()
}

/// The type, then the field with an arm from each module that works on it,
/// then its other fields.
fn spread(
    owner: &str,
    field: &str,
    modules: &[(usize, String)],
    others: &[String],
    paint: &Paint,
) -> Vec<String> {
    let mut lines = vec![owner.to_owned()];
    let digits = modules
        .iter()
        .map(|(count, _)| count.to_string().len())
        .max()
        .unwrap_or(0);
    let under = " ".repeat(field.chars().count() + 3);
    for (index, (count, module)) in modules.iter().enumerate() {
        let (lead, arm) = match index {
            0 => (
                format!("  {} ", paint.bold(field)),
                if modules.len() == 1 {
                    "◀━━━"
                } else {
                    "◀━┳━"
                },
            ),
            _ if index + 1 == modules.len() => (under.clone(), "  ┗━"),
            _ => (under.clone(), "  ┣━"),
        };
        lines.push(format!(
            "{lead}{} {count:>digits$}  {module}",
            paint.error(arm)
        ));
    }
    lines.extend(others.iter().map(|other| format!("  {other}")));
    lines
}

/// A bar for how many functions a module shows and hides, and for each list
/// too long, each with a line at what a reader holds in mind; past it, a bar
/// of what is shown is heavy.
fn bars(shown: usize, hidden: usize, lists: &[(String, usize)], paint: &Paint) -> Vec<String> {
    let rows = [("shows", shown, true), ("hides", hidden, false)]
        .into_iter()
        .chain(
            lists
                .iter()
                .map(|(name, length)| (name.as_str(), *length, true)),
        );
    let rows: Vec<(&str, usize, bool)> = rows.collect();
    let width = rows
        .iter()
        .map(|(name, ..)| name.chars().count())
        .max()
        .unwrap_or(0);
    let limit = HELD_IN_MIND + 1;
    let mut lines = vec![format!("{:width$}  {HELD_IN_MIND:>limit$}", "")];
    for (name, length, judged) in rows {
        let inside = length.min(HELD_IN_MIND);
        let past = length - inside;
        let mut beyond = "█".repeat(past.min(BAR - HELD_IN_MIND));
        if past > BAR - HELD_IN_MIND {
            beyond.push('⋯');
        }
        if judged && past > 0 {
            beyond = paint.error(&beyond);
        }
        let (full, empty) = ("█".repeat(inside), " ".repeat(HELD_IN_MIND - inside));
        lines.push(format!("{name:width$}  {full}{empty}│{beyond} {length}"));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::Location;

    fn plain(picture: &Picture) -> Vec<String> {
        draw(picture, &Paint(false))
    }

    fn step(member: &str, file: &str, line: u32, uses: &str) -> Step {
        Step {
            member: member.into(),
            site: Location {
                file: file.into(),
                line,
            },
            uses: uses.into(),
        }
    }

    #[test]
    fn a_circle_runs_down_its_ties_and_back_up_from_the_last() {
        let picture = Picture::Circle {
            steps: vec![
                step("kinds", "src/kinds.rs", 10, "Stream::new uses BACKLOG_MOST"),
                step("socket", "src/net/socket.rs", 5, "listen uses Stream"),
            ],
            rest: Some("not drawn: 1 more module".into()),
        };
        assert_eq!(
            plain(&picture),
            [
                "    ┌─▶ kinds",
                "    │     ┃ kinds.rs:10      Stream::new uses BACKLOG_MOST",
                "    │     ▼",
                "    │   socket",
                "    │     │ net/socket.rs:5  listen uses Stream",
                "    └─────┘",
                "    not drawn: 1 more module",
            ]
        );
    }

    #[test]
    fn a_spread_draws_an_arm_from_each_module_into_the_field() {
        let spread = |modules: Vec<(usize, String)>| Picture::Spread {
            owner: "q::RunQueue".into(),
            field: "items".into(),
            modules,
            others: vec!["kept: limit".into()],
        };
        assert_eq!(
            plain(&spread(vec![(1, "q::app".into())])),
            [
                "    q::RunQueue",
                "      items ◀━━━ 1  q::app",
                "      kept: limit",
            ]
        );
        let modules = vec![
            (12, "q::sched".into()),
            (3, "q::app".into()),
            (2, "in 2 more modules".into()),
        ];
        assert_eq!(
            plain(&spread(modules)),
            [
                "    q::RunQueue",
                "      items ◀━┳━ 12  q::sched",
                "              ┣━  3  q::app",
                "              ┗━  2  in 2 more modules",
                "      kept: limit",
            ]
        );
    }

    #[test]
    fn a_bar_crosses_the_line_at_seven_and_ends_short_of_eighty_columns() {
        let picture = Picture::Lists {
            shown: 50,
            hidden: 3,
            lists: vec![("Store".into(), 9)],
        };
        assert_eq!(
            plain(&picture),
            [
                "                  7",
                "    shows  ███████│█████████████████████████████████⋯ 50",
                "    hides  ███    │ 3",
                "    Store  ███████│██ 9",
            ]
        );
    }
}
