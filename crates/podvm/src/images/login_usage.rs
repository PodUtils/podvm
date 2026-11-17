//! The usage of `podvm login` and `podvm logout`, out of `list.rs` for the
//! limit of 500 lines of a source file.

pub const LOGIN_USAGE: &str = "\
usage: podvm login [-u|--username USER] [--password-stdin] [SERVER]

  Log in to a registry. The password arrives on stdin, never as an
  argument: an argument lives in the shell history and the process table.
  There is no interactive prompt; an automated caller pipes the password
  in, and a runtime that blocks on input is the hang AGENTS.md forbids.

  Stores in ~/.docker/config.json, the file docker writes, or through the
  configured credential helper where one is named for the server. SERVER
  defaults to https://index.docker.io/v1/, docker's canonical key.

  The stored login is used, never printed: pulls send it, errors name the
  registry, and result files never carry it (TODO/image.md T-0209).
";

pub const LOGOUT_USAGE: &str = "\
usage: podvm logout [SERVER]

  Log out of a registry: remove the login `podvm login` stored, from
  the file holding it or through the configured credential helper
  where one is named for the server. SERVER defaults to
  https://index.docker.io/v1/, docker's canonical key.

  A logout of a server with none stored says that it is not logged in
  and exits 1, so that a mistyped name shows (TODO/image.md T-0209).
";
