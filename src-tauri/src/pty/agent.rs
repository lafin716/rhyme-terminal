use std::collections::{HashMap, HashSet, VecDeque};

use sysinfo::System;

use super::AgentKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessEntry {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub image_name: String,
    pub command_args: Vec<String>,
    pub started_at: u64,
}

pub fn agent_for_shell(shell_pid: u32, processes: &[ProcessEntry]) -> AgentKind {
    let mut children_by_parent: HashMap<u32, Vec<&ProcessEntry>> = HashMap::new();
    for process in processes {
        if let Some(parent_pid) = process.parent_pid {
            children_by_parent
                .entry(parent_pid)
                .or_default()
                .push(process);
        }
    }

    let mut descendants = Vec::new();
    let mut pending = VecDeque::from([shell_pid]);
    let mut seen = HashSet::from([shell_pid]);

    while let Some(parent_pid) = pending.pop_front() {
        if let Some(children) = children_by_parent.get(&parent_pid) {
            for &process in children {
                if seen.insert(process.pid) {
                    descendants.push(process);
                    pending.push_back(process.pid);
                }
            }
        }
    }

    descendants
        .into_iter()
        .filter_map(|process| agent_for_process(process).map(|agent| (process.started_at, agent)))
        .max_by_key(|(started_at, _)| *started_at)
        .map(|(_, agent)| agent)
        .unwrap_or(AgentKind::Terminal)
}

pub fn process_snapshot() -> Vec<ProcessEntry> {
    let system = System::new_all();

    system
        .processes()
        .values()
        .map(|process| ProcessEntry {
            pid: process.pid().as_u32(),
            parent_pid: process.parent().map(|pid| pid.as_u32()),
            image_name: process.name().to_string_lossy().into_owned(),
            command_args: process
                .cmd()
                .iter()
                .map(|argument| argument.to_string_lossy())
                .map(|argument| argument.into_owned())
                .collect(),
            started_at: process.start_time(),
        })
        .collect()
}

fn agent_for_process(process: &ProcessEntry) -> Option<AgentKind> {
    [AgentKind::Claude, AgentKind::Codex]
        .into_iter()
        .find(|agent| {
            executable_matches(&process.image_name, agent.executable_name())
                || wrapper_target(process)
                    .is_some_and(|target| wrapper_target_matches(target, *agent))
        })
}

fn wrapper_target(process: &ProcessEntry) -> Option<&str> {
    let arguments = command_arguments(process);

    if executable_matches(&process.image_name, "node") {
        return arguments.first().map(String::as_str);
    }

    if executable_matches(&process.image_name, "cmd") {
        return argument_after(arguments, &["/c", "/k"]);
    }

    if executable_matches(&process.image_name, "powershell")
        || executable_matches(&process.image_name, "pwsh")
    {
        return argument_after(arguments, &["-file", "-f"]);
    }

    None
}

fn command_arguments(process: &ProcessEntry) -> &[String] {
    match process.command_args.first() {
        Some(program) if same_executable(program, &process.image_name) => {
            &process.command_args[1..]
        }
        _ => &process.command_args,
    }
}

fn argument_after<'a>(arguments: &'a [String], switches: &[&str]) -> Option<&'a str> {
    arguments.windows(2).find_map(|pair| {
        switches
            .iter()
            .any(|switch| pair[0].eq_ignore_ascii_case(switch))
            .then_some(pair[1].as_str())
    })
}

fn wrapper_target_matches(candidate: &str, agent: AgentKind) -> bool {
    let executable_name = agent.executable_name();
    let file_name = file_name(candidate);
    let name_matches = ["", ".exe", ".cmd", ".ps1", ".js", ".mjs"]
        .into_iter()
        .any(|suffix| file_name.eq_ignore_ascii_case(&format!("{executable_name}{suffix}")));
    if name_matches {
        return true;
    }

    let normalized = candidate.replace('\\', "/").to_ascii_lowercase();
    match agent {
        AgentKind::Claude => normalized.contains("/@anthropic-ai/claude-code/"),
        AgentKind::Codex => normalized.contains("/@openai/codex/"),
        AgentKind::Terminal => false,
    }
}

fn executable_matches(candidate: &str, executable_name: &str) -> bool {
    let file_name = file_name(candidate);
    file_name.eq_ignore_ascii_case(executable_name)
        || file_name.eq_ignore_ascii_case(&format!("{executable_name}.exe"))
}

fn same_executable(left: &str, right: &str) -> bool {
    file_name(left).eq_ignore_ascii_case(file_name(right))
}

fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::{agent_for_shell, AgentKind, ProcessEntry};

    fn entry(
        pid: u32,
        parent_pid: Option<u32>,
        image_name: &str,
        command_args: &[&str],
        started_at: u64,
    ) -> ProcessEntry {
        ProcessEntry {
            pid,
            parent_pid,
            image_name: image_name.to_string(),
            command_args: command_args
                .iter()
                .map(|argument| argument.to_string())
                .collect(),
            started_at,
        }
    }

    #[test]
    fn classifies_a_nested_codex_descendant_case_insensitively() {
        let processes = [
            entry(10, None, "pwsh.exe", &["pwsh"], 1),
            entry(11, Some(10), "cmd.exe", &["cmd"], 2),
            entry(12, Some(11), "CoDeX.ExE", &["codex"], 3),
        ];

        assert_eq!(agent_for_shell(10, &processes), AgentKind::Codex);
    }

    #[test]
    fn classifies_a_claude_descendant() {
        let processes = [
            entry(10, None, "pwsh.exe", &["pwsh"], 1),
            entry(11, Some(10), "claude.exe", &["claude"], 2),
        ];

        assert_eq!(agent_for_shell(10, &processes), AgentKind::Claude);
    }

    #[test]
    fn matches_a_mixed_case_agent_command_line() {
        let processes = [
            entry(10, None, "pwsh.exe", &["pwsh"], 1),
            entry(11, Some(10), "node.exe", &["CoDeX.ExE", "--resume"], 2),
        ];

        assert_eq!(agent_for_shell(10, &processes), AgentKind::Codex);
    }

    #[test]
    fn ignores_agent_names_in_regular_command_arguments() {
        for (image_name, command_args) in [
            ("ping.exe", &["ping", "codex"][..]),
            ("rg.exe", &["rg", "claude"][..]),
            ("node.exe", &["node", "worker.js", "codex"][..]),
        ] {
            let processes = [
                entry(10, None, "pwsh.exe", &["pwsh"], 1),
                entry(11, Some(10), image_name, command_args, 2),
            ];

            assert_eq!(
                agent_for_shell(10, &processes),
                AgentKind::Terminal,
                "{command_args:?} must not be classified as an agent"
            );
        }
    }

    #[test]
    fn classifies_known_wrapper_script_positions() {
        for (image_name, command_args, expected) in [
            (
                "node.exe",
                &[
                    "node.exe",
                    r#"C:\tools\node_modules\@openai\codex\bin\codex.js"#,
                    "--resume",
                ][..],
                AgentKind::Codex,
            ),
            (
                "node.exe",
                &[
                    "node.exe",
                    r#"C:\tools\node_modules\@anthropic-ai\claude-code\cli.js"#,
                    "--resume",
                ][..],
                AgentKind::Claude,
            ),
            (
                "cmd.exe",
                &["cmd.exe", "/D", "/C", r#"C:\tools\claude.cmd"#, "--resume"][..],
                AgentKind::Claude,
            ),
            (
                "powershell.exe",
                &[
                    "powershell.exe",
                    "-File",
                    r#"C:\tools\codex.ps1"#,
                    "--resume",
                ][..],
                AgentKind::Codex,
            ),
        ] {
            let processes = [
                entry(10, None, "pwsh.exe", &["pwsh"], 1),
                entry(11, Some(10), image_name, command_args, 2),
            ];

            assert_eq!(
                agent_for_shell(10, &processes),
                expected,
                "{command_args:?} should classify its wrapper target"
            );
        }
    }

    #[test]
    fn selects_the_newest_matching_descendant() {
        let processes = [
            entry(10, None, "pwsh.exe", &["pwsh"], 1),
            entry(11, Some(10), "node.exe", &["claude"], 2),
            entry(12, Some(10), "node.exe", &["codex"], 3),
        ];

        assert_eq!(agent_for_shell(10, &processes), AgentKind::Codex);
    }

    #[test]
    fn falls_back_to_terminal_without_a_matching_descendant() {
        let processes = [entry(10, None, "pwsh.exe", &["pwsh"], 1)];

        assert_eq!(agent_for_shell(10, &processes), AgentKind::Terminal);
    }
}
