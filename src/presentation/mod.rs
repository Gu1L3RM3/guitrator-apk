pub mod cli;
pub mod gui;
pub mod mcp;

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
}
