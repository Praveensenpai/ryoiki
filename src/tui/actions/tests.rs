use super::INSTALL_HOOKS_ARGS;

#[test]
fn install_hooks_uses_the_real_cli_subcommand() {
    assert_eq!(INSTALL_HOOKS_ARGS, ["notify", "install-hooks"]);
}
