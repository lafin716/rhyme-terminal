use crate::pty::{
    agent::{agent_for_process, ProcessEntry, ProcessTree},
    manager::SessionManager,
};
use uuid::Uuid;

/// Capture identities before interrupt. Descendants remain owned even if the
/// CLI exits and the OS reparents a tool process.
pub fn capture(manager: &SessionManager, sid: Uuid, helper: Option<u32>) -> Vec<ProcessEntry> {
    let Some(shell) = manager.sessions.lock().get(&sid).map(|s| s.shell_pid) else {
        return vec![];
    };
    let monitor = manager.runtime.lock();
    let processes = &monitor.processes;
    let tree = ProcessTree::new(processes);
    let mut roots: Vec<_> = tree
        .descendants([shell])
        .into_iter()
        .filter(|p| agent_for_process(p).is_some() || Some(p.pid) == helper)
        .collect();
    let children = tree.descendants(roots.iter().map(|p| p.pid));
    roots.extend(children);
    roots.sort_by_key(|p| p.pid);
    roots.dedup_by_key(|p| p.pid);
    roots.into_iter().cloned().collect()
}
pub fn alive(targets: &[ProcessEntry], processes: &[ProcessEntry]) -> bool {
    targets.iter().any(|old| {
        processes
            .iter()
            .any(|p| p.pid == old.pid && p.started_at == old.started_at)
    })
}
pub fn refresh(targets: &mut Vec<ProcessEntry>, processes: &[ProcessEntry]) {
    let roots: Vec<_> = targets
        .iter()
        .filter(|old| {
            processes
                .iter()
                .any(|p| p.pid == old.pid && p.started_at == old.started_at)
        })
        .map(|p| p.pid)
        .collect();
    let tree = ProcessTree::new(processes);
    targets.extend(tree.descendants(roots).into_iter().cloned());
    targets.sort_by_key(|p| (p.pid, p.started_at));
    targets.dedup_by_key(|p| (p.pid, p.started_at));
}
pub fn signal(targets: &[ProcessEntry], force: bool) {
    let system = sysinfo::System::new_all();
    for target in targets.iter().rev() {
        if let Some(p) = system
            .process(sysinfo::Pid::from_u32(target.pid))
            .filter(|p| p.start_time() == target.started_at)
        {
            let result = p.kill_with(if force {
                sysinfo::Signal::Kill
            } else {
                sysinfo::Signal::Term
            });
            if result.is_none() {
                let _ = p.kill();
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_reused_pid_is_not_the_old_agent() {
        let old = ProcessEntry {
            pid: 42,
            parent_pid: Some(1),
            image_name: "codex".into(),
            command_args: vec![],
            started_at: 10,
        };
        let mut new = old.clone();
        new.started_at = 11;
        assert!(!alive(&[old.clone()], &[new]));
        assert!(alive(&[old.clone()], &[old]));
    }

    #[test]
    fn retains_children_created_during_interrupt_even_after_reparenting() {
        let helper = ProcessEntry {
            pid: 42,
            parent_pid: Some(1),
            image_name: "bridge".into(),
            command_args: vec![],
            started_at: 10,
        };
        let mut child = ProcessEntry {
            pid: 43,
            parent_pid: Some(42),
            image_name: "codex".into(),
            command_args: vec![],
            started_at: 11,
        };
        let mut targets = vec![helper.clone()];
        refresh(&mut targets, &[helper, child.clone()]);
        child.parent_pid = Some(1);
        assert!(alive(&targets, &[child]));
    }

    #[cfg(windows)]
    #[test]
    fn terminates_only_the_captured_test_child() {
        use std::os::windows::process::CommandExt;
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        let mut child = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 30",
            ])
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let system = sysinfo::System::new_all();
        let process = system.process(sysinfo::Pid::from_u32(child.id())).unwrap();
        let target = ProcessEntry {
            pid: child.id(),
            parent_pid: Some(std::process::id()),
            image_name: "powershell.exe".into(),
            command_args: vec![],
            started_at: process.start_time(),
        };
        signal(&[target], false);
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() > deadline {
                let _ = child.kill();
                panic!("test child did not terminate");
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}
