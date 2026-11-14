use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use elemental::launcher::LaunchCommand;
use ratail::Redraw;
use tokio::process::{Child, Command};

const SESSION_HISTORY: usize = 10;
const REDACTED: &str = "***";
const SECRET_ARGUMENTS: &[&str] = &["--accessToken", "--clientId", "--xuid", "--uuid"];

pub struct LaunchSession {
    pub log_path: PathBuf,
    pub started_at: SystemTime,
}

impl LaunchSession {
    pub fn create(
        storage_root: &Path,
        instance_name: &str,
        command: &LaunchCommand,
    ) -> Result<Self> {
        let log_root = storage_root
            .join("logs")
            .join("particle")
            .join(sanitize_component(instance_name));
        fs::create_dir_all(&log_root)
            .with_context(|| format!("failed to create log directory: {}", log_root.display()))?;
        prune_logs(&log_root, SESSION_HISTORY)?;

        let started_at = SystemTime::now();
        let log_path = log_root.join(format!("{}.log", timestamp(started_at)));
        let mut file = File::create(&log_path)
            .with_context(|| format!("failed to create launch log: {}", log_path.display()))?;
        write_header(&mut file, instance_name, started_at, command)?;

        Ok(Self {
            log_path,
            started_at,
        })
    }

    pub fn spawn(&self, command: &LaunchCommand) -> Result<Child> {
        let stdout = OpenOptions::new()
            .append(true)
            .open(&self.log_path)
            .with_context(|| format!("failed to open launch log: {}", self.log_path.display()))?;
        let stderr = stdout
            .try_clone()
            .context("failed to clone launch log handle")?;

        let mut process = Command::new(command.program());
        process
            .args(command.arguments())
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(false);

        if let Some(cwd) = &command.cwd {
            process.current_dir(cwd);
        }
        for (key, value) in &command.env {
            process.env(key, value);
        }

        #[cfg(windows)]
        process.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);

        process.spawn().with_context(|| {
            format!(
                "failed to spawn {} for instance",
                command.program().display()
            )
        })
    }
}

pub fn supervise(mut child: Child, session: LaunchSession, redraw: Redraw) {
    tokio::spawn(async move {
        let status = child.wait().await;
        let _ = append_footer(&session.log_path, &status);
        redraw.trigger();
    });
}

fn write_header(
    file: &mut File,
    instance_name: &str,
    started_at: SystemTime,
    command: &LaunchCommand,
) -> Result<()> {
    writeln!(file, "Particle launch session")?;
    writeln!(file, "instance: {instance_name}")?;
    writeln!(file, "started_at: {started_at:?}")?;
    writeln!(file, "program: {}", command.program().display())?;
    if let Some(cwd) = &command.cwd {
        writeln!(file, "cwd: {}", cwd.display())?;
    }
    writeln!(file, "main_class: {}", command.main_class)?;
    writeln!(file, "jvm_arguments: {}", command.jvm_arguments.join(" "))?;
    writeln!(
        file,
        "game_arguments: {}",
        redact_arguments(&command.game_arguments).join(" ")
    )?;
    writeln!(
        file,
        "env_keys: {}",
        command
            .env
            .keys()
            .map(String::as_str)
            .collect::<Vec<&str>>()
            .join(",")
    )?;
    writeln!(file, "--- process output ---")?;
    file.flush().context("failed to flush launch log header")
}

fn append_footer(
    log_path: &Path,
    status: &std::io::Result<std::process::ExitStatus>,
) -> Result<()> {
    let mut file = OpenOptions::new()
        .append(true)
        .open(log_path)
        .with_context(|| format!("failed to open launch log: {}", log_path.display()))?;
    match status {
        Ok(status) => writeln!(file, "--- process exited: {status} ---")?,
        Err(error) => writeln!(file, "--- process wait failed: {error} ---")?,
    }
    file.flush().context("failed to flush launch log footer")
}

fn redact_arguments(arguments: &[String]) -> Vec<String> {
    let mut redacted = Vec::with_capacity(arguments.len());
    let mut hide_next = false;
    for argument in arguments {
        if hide_next {
            redacted.push(REDACTED.to_owned());
            hide_next = false;
            continue;
        }
        redacted.push(argument.clone());
        hide_next = SECRET_ARGUMENTS.contains(&argument.as_str());
    }
    redacted
}

fn prune_logs(log_root: &Path, keep: usize) -> Result<()> {
    let keep = keep.max(1);
    let mut logs = fs::read_dir(log_root)
        .with_context(|| format!("failed to list log directory: {}", log_root.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<PathBuf>>>()
        .with_context(|| format!("failed to list log directory: {}", log_root.display()))?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "log"))
        .collect::<Vec<PathBuf>>();
    logs.sort();

    let remove_count = logs.len().saturating_sub(keep - 1);
    for path in logs.into_iter().take(remove_count) {
        fs::remove_file(&path)
            .with_context(|| format!("failed to remove old launch log: {}", path.display()))?;
    }
    Ok(())
}

fn timestamp(time: SystemTime) -> u128 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn sanitize_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => character,
            _ => '_',
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "instance".to_owned()
    } else {
        sanitized
    }
}

#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
