use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("Command failed: {command}\nOutput: {output}")]
    Failed { command: String, output: String },

    #[error("Timed out after {duration:?}: {command}")]
    Timeout { command: String, duration: Duration },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct CommandExecutor;

impl CommandExecutor {
    pub async fn execute(
        command: &str,
        args: &[&str],
        env_vars: &[(&str, &str)],
        timeout_duration: Duration,
    ) -> Result<String, CommandError> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        let result = timeout(timeout_duration, cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = format!("{stdout}\n{stderr}");

                if output.status.success() {
                    Ok(combined.trim().to_string())
                } else {
                    Err(CommandError::Failed {
                        command: format!("{} {:?}", command, args),
                        output: combined,
                    })
                }
            }
            Ok(Err(e)) => Err(CommandError::Io(e)),
            Err(_) => Err(CommandError::Timeout {
                command: format!("{} {:?}", command, args),
                duration: timeout_duration,
            }),
        }
    }

    pub async fn execute_wine_command(
        wineprefix: &str,
        winearch: &str,
        command: &str,
        args: &[&str],
        timeout_duration: Duration,
    ) -> Result<String, CommandError> {
        Self::execute(
            "wine",
            &[command].iter().chain(args.iter()).copied().collect::<Vec<_>>().as_slice(),
            &[("WINEPREFIX", wineprefix), ("WINEARCH", winearch)],
            timeout_duration,
        )
        .await
    }

    pub async fn execute_winetricks(
        wineprefix: &str,
        winearch: &str,
        component: &str,
        timeout_duration: Duration,
    ) -> Result<String, CommandError> {
        Self::execute(
            "winetricks",
            &["-q", component],
            &[("WINEPREFIX", wineprefix), ("WINEARCH", winearch)],
            timeout_duration,
        )
        .await
    }

    /// Launches a process detached without waiting for termination.
    /// Appropriate for GUI applications (like Wine + Office) that the user closes manually.
    /// stdio is set to null to prevent the child from holding references to parent's handles.
    pub fn spawn_detached(
        command: &str,
        args: &[&str],
        env_vars: &[(&str, &str)],
    ) -> Result<tokio::process::Child, CommandError> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());

        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        cmd.spawn().map_err(CommandError::Io)
    }
}
