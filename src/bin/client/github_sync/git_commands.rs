use std::path::Path;

pub fn git_exists() -> bool {
    std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_ok()
}

pub fn git_clone(repo_url: &str, target_dir: &Path) -> Result<(), String> {
    let status = std::process::Command::new("git")
        .arg("clone")
        .arg(repo_url)
        .arg(target_dir)
        .status()
        .map_err(|e| format!("Failed to execute git clone command: {}", e))?;

    if !status.success() {
        return Err(format!(
            "Git clone command failed with exit code: {}",
            status.code().unwrap_or(-1)
        ));
    }

    Ok(())
}


pub fn add_all_files_to_git() -> Result<(), String> {
    let status = std::process::Command::new("git")
        .arg("add")
        .arg(".")
        .status()
        .map_err(|e| format!("Failed to execute git add command: {}", e))?;

    if !status.success() {
        return Err(format!(
            "Git add command failed with exit code: {}",
            status.code().unwrap_or(-1)
        ));
    }

    Ok(())
}

// Returns Ok(true) if there are changes to commit
pub fn git_commit(commit_message: &str) -> Result<bool, String> {
    let status = std::process::Command::new("git")
        .arg("commit")
        .arg("-m")
        .arg(commit_message)
        .status()
        .map_err(|e| format!("Failed to execute git commit command: {}", e))?;

    if !status.success() {
        // Exit code 1 - nothing to commit
        if let Some(1) = status.code() {
            return Ok(false);
        }

        return Err(format!(
            "Git commit command failed with exit code: {}",
            status.code().unwrap_or(-1)
        ));
    }

    Ok(true)
}


pub fn git_force_push() -> Result<(), String> {
    let status = std::process::Command::new("git")
        .arg("push")
        .arg("--force")
        .status()
        .map_err(|e| format!("Failed to execute git push command: {}", e))?;

    if !status.success() {
        return Err(format!(
            "Git push command failed with exit code: {}",
            status.code().unwrap_or(-1)
        ));
    }

    Ok(())
}