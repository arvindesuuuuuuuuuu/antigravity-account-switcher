use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::paths::find_language_server_log;

#[derive(Debug, Clone)]
pub struct LanguageServerEndpoint {
    pub pid: u32,
    pub csrf_token: String,
    pub http_port: Option<u16>,
    pub https_port: Option<u16>,
}

/// Checks if the Antigravity desktop Electron app is running
pub fn is_antigravity_running() -> bool {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All);

    for (_pid, process) in system.processes() {
        let name = process.name().to_string_lossy().to_ascii_lowercase();
        let cmd = process
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");

        // Strictly target Antigravity (exclude IDE)
        if (name.contains("antigravity") || cmd.contains("antigravity"))
            && !name.contains("antigravity ide")
            && !cmd.contains("antigravity ide")
            && !cmd.contains("antigravity-ide")
        {
            return true;
        }
    }
    false
}

/// Discovers PIDs of `language_server` subprocesses belonging to Antigravity
pub fn get_language_server_pids() -> Vec<u32> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All);

    let mut target_pids = Vec::new();

    for (pid, process) in system.processes() {
        let name = process.name().to_string_lossy().to_string();
        let cmd = process
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");

        // Must be language_server
        let is_ls = name.contains("language_server") || cmd.contains("language_server");
        if !is_ls {
            continue;
        }

        // Must target Antigravity (classic/hub) and NOT Antigravity IDE
        let is_antigravity = cmd.contains("--override_ide_name antigravity")
            || cmd.contains("--app_data_dir antigravity")
            || cmd.contains("/antigravity/resources/bin/language_server")
            || cmd.contains(r"\Antigravity\resources\bin\language_server");

        let is_not_ide = !cmd.contains("antigravity ide") && !cmd.contains("antigravity-ide");

        if is_antigravity && is_not_ide {
            target_pids.push(pid.as_u32());
        }
    }

    target_pids
}

/// Discovers the running Antigravity `language_server` endpoint details (PID, CSRF token, HTTP/HTTPS ports)
pub fn get_language_server_endpoint() -> Option<LanguageServerEndpoint> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All);

    let mut found_csrf: Option<String> = None;
    let mut found_pid: Option<u32> = None;

    for (pid, process) in system.processes() {
        let name = process.name().to_string_lossy().to_string();
        let cmd_vec: Vec<String> = process
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        let cmd_joined = cmd_vec.join(" ");

        let is_ls = name.contains("language_server") || cmd_joined.contains("language_server");
        if !is_ls {
            continue;
        }

        let is_antigravity = cmd_joined.contains("--override_ide_name antigravity")
            || cmd_joined.contains("--app_data_dir antigravity")
            || cmd_joined.contains("/antigravity/resources/bin/language_server")
            || cmd_joined.contains(r"\Antigravity\resources\bin\language_server");

        let is_not_ide = !cmd_joined.contains("antigravity ide") && !cmd_joined.contains("antigravity-ide");

        if is_antigravity && is_not_ide {
            found_pid = Some(pid.as_u32());
            for (i, arg) in cmd_vec.iter().enumerate() {
                if arg == "--csrf_token" && i + 1 < cmd_vec.len() {
                    found_csrf = Some(cmd_vec[i + 1].clone());
                    break;
                } else if arg.starts_with("--csrf_token=") {
                    found_csrf = Some(arg.trim_start_matches("--csrf_token=").to_string());
                    break;
                }
            }
            if found_csrf.is_some() {
                break;
            }
        }
    }

    let csrf = found_csrf?;
    let pid = found_pid.unwrap_or(0);

    let mut http_port = None;
    let mut https_port = None;

    if let Some(log_path) = find_language_server_log() {
        if let Ok(content) = std::fs::read_to_string(&log_path) {
            for line in content.lines().rev() {
                if https_port.is_none() && line.contains("Language server listening on random port at ") && line.contains("for HTTPS") {
                    if let Some(pos) = line.find("random port at ") {
                        let sub = &line[pos + 15..];
                        if let Some(end) = sub.find(" for HTTPS") {
                            if let Ok(p) = sub[..end].trim().parse::<u16>() {
                                https_port = Some(p);
                            }
                        }
                    }
                }
                if http_port.is_none() && line.contains("Language server listening on random port at ") && line.contains("for HTTP") && !line.contains("for HTTPS") {
                    if let Some(pos) = line.find("random port at ") {
                        let sub = &line[pos + 15..];
                        if let Some(end) = sub.find(" for HTTP") {
                            if let Ok(p) = sub[..end].trim().parse::<u16>() {
                                http_port = Some(p);
                            }
                        }
                    }
                }
                if http_port.is_some() && https_port.is_some() {
                    break;
                }
            }
        }
    }

    Some(LanguageServerEndpoint {
        pid,
        csrf_token: csrf,
        http_port,
        https_port,
    })
}

/// Kills `language_server` child processes to trigger supervisor hot-reload
pub fn kill_language_server_subprocesses() -> Result<usize, String> {
    let pids = get_language_server_pids();
    if pids.is_empty() {
        return Ok(0);
    }

    let mut killed_count = 0;
    for pid in &pids {
        println!("[HotSwitch] Terminating Antigravity language_server PID {}", pid);
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .output();
            killed_count += 1;
        }

        #[cfg(not(target_os = "windows"))]
        {
            let mut system = System::new();
            system.refresh_processes(ProcessesToUpdate::All);
            if let Some(process) = system.process(Pid::from_u32(*pid)) {
                if process.kill() {
                    killed_count += 1;
                }
            } else {
                let _ = std::process::Command::new("kill")
                    .args(["-9", &pid.to_string()])
                    .output();
                killed_count += 1;
            }
        }
    }

    Ok(killed_count)
}
