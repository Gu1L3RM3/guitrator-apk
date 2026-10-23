pub mod cli;
pub mod details;
pub mod format;
pub mod gui;
pub mod help;
pub mod mcp;
pub mod report;

/// CLI name and arguments that register `exe mcp` as an MCP server; None for an unknown target.
pub fn setup_mcp_command(target: Option<&str>, exe: &str) -> Option<(&'static str, Vec<String>)> {
    let (cli, scope): (&'static str, &[&str]) = match target? {
        "claude" => ("claude", &["--scope", "user"]),
        "codex" => ("codex", &[]),
        _ => return None,
    };
    let mut args: Vec<String> = ["mcp", "add"].iter().map(|s| s.to_string()).collect();
    args.extend(scope.iter().map(|s| s.to_string()));
    args.extend(["guitrator-apk", "--", exe, "mcp"].map(String::from));
    Some((cli, args))
}

/// What to run on the host to register the Flatpak build as an MCP server. `setup-mcp` cannot
/// do it from inside the sandbox: `claude` and `codex` do not exist there, and neither does
/// this binary's `/app/bin` path on the host.
pub fn flatpak_mcp_help(app_id: &str) -> String {
    format!(
        "setup-mcp cannot run inside the Flatpak sandbox. Run one of these in a normal terminal instead:\n  \
         claude mcp add --scope user guitrator-apk -- flatpak run {app_id} mcp\n  \
         codex mcp add guitrator-apk -- flatpak run {app_id} mcp"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_claude_and_codex_commands() {
        let (cli, args) = setup_mcp_command(Some("claude"), "/x/g").unwrap();
        assert_eq!(cli, "claude");
        assert_eq!(
            args.join(" "),
            "mcp add --scope user guitrator-apk -- /x/g mcp"
        );
        let (cli, args) = setup_mcp_command(Some("codex"), "/x/g").unwrap();
        assert_eq!(
            (cli, args.join(" ").as_str()),
            ("codex", "mcp add guitrator-apk -- /x/g mcp")
        );
        assert!(setup_mcp_command(Some("vim"), "/x/g").is_none());
        assert!(setup_mcp_command(None, "/x/g").is_none());
    }

    #[test]
    fn flatpak_help_gives_a_host_command_for_each_client() {
        let help = flatpak_mcp_help("org.example.App");
        assert!(help.contains(
            "claude mcp add --scope user guitrator-apk -- flatpak run org.example.App mcp"
        ));
        assert!(help.contains("codex mcp add guitrator-apk -- flatpak run org.example.App mcp"));
    }
}
