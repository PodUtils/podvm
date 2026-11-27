//! `podvm run`: milestone M3.
//!
//! [`TODO/milestones.md`](../../../TODO/milestones.md) T-1104,
//! [`TODO/enter.md`](../../../TODO/enter.md) T-0501 to T-0506,
//! [`TODO/cli.md`](../../../TODO/cli.md) T-0802.
//!
//! ⭐ **This verb is the product requirement the whole specification exists
//! for**: an agent that knows `docker` needs zero new knowledge. So its
//! argument surface, its output channels and its exit codes are docker's, and
//! where podvm cannot honour something it says so in one line rather than
//! pretending.

use std::io::Write;

use podvm_enter::{binfmt, Fds, Plan, RootDir};
use podvm_image::error::{EXIT_CLI_ERROR, EXIT_FLAG_ERROR};
use podvm_image::platform::Platform;
use podvm_image::transport::Policy;

/// ⭐ The option block, with no first line, because `create` is served by this
/// same parser and its usage must not say `run`. [`usage`] builds that line from
/// the verb the caller actually typed. One block, so a flag documented here
/// cannot be missing from the other verb's help. TODO/cli.md T-0801.
pub const RUN_OPTIONS: &str = "\
  -d, --detach     start the container and print its id, and do not wait
  --name NAME      a name for the container.
                   note: Refused if one already has it.
                   A foreground run leaves a record under it: `ps -a` lists
                   it as Exited and `logs` reads its output
                   (TODO/supervise.md T-1421). `container prune` removes
                   each exited record and its tree (T-1848)
  --rm             remove the extracted rootfs when the payload exits,
                   unless a container record references it: the rootfs is
                   shared, and a failed run must not delete what `keeper`
                   needs (TODO/image.md T-1322)
  -e, --env K=V    set an environment variable. Repeatable; a later one wins.
                   The container record keeps each value under the store
                   root (mode 0700): pass a secret in a file, not here
                   (TODO/supervise.md T-1715)
  --env-file F     read KEY=VALUE lines from F into the environment.
                   Repeatable; entries load at the flag's position, so a
                   later -e wins over the file; a value keeps its quotes
                   and its trailing space; a name with white space (`export
                   I=9`, `FOO BAR=2`) or a digit first is refused with the
                   line number (TODO/cli.md T-0801, T-1828, T-2108)
  --env-file0 F    read KEY=VALUE entries from F, each ended by a NUL (as
                   `env -0` writes them), so a value can hold a newline;
                   the same order rule as --env-file (TODO/cli.md T-1971)
  -w, --workdir D  working directory inside the container
  --device H[:G[:P]]
                   map a host device or file into the container at G, as
                   a -v of one file: a bind on the namespace rung, the
                   path table on the userland rung. Repeatable. P takes
                   r, w and m; r alone is read-only, and the namespace
                   rung refuses r alone for a device node (a read-only
                   bind does not refuse its writes); w alone keeps the
                   descriptor serve of the interposer; m parses and
                   grants nothing (TODO/enter.md T-0501, T-1939)
  -v, --volume H:G[:rw|:ro|:overlay|:mapped]
                   show the host path H at the guest path G. Repeatable. A
                   path rewrite through PODVM_MAPS on the userland loader
                   family, not a mount: a write lands in H, and :ro refuses
                   a write (TODO/cli.md T-1655). A 9p mount on the machine
                   tier. A bind mount on the namespace rung, which
                   /proc/mounts lists. A -w or a first program inside G
                   resolves in H (TODO/enter.md T-1653). :overlay, the
                   namespace rung only (also -d, create and start): the
                   directory H is the read-only lower layer of an overlayfs
                   mount on G, and each write, chmod, unlink or rename goes
                   to an upper layer in the tree of the container, which
                   rm (or the end of run --rm) removes; no other volume may
                   be at, over or under G, nor G under /tmp, /dev, /proc or
                   /sys. A file of H whose owner the user namespace does
                   not map (a file of root on a host with no root for you,
                   or of another user) can be read, and a write, chmod or
                   rename of it fails with EOVERFLOW (\"Value too large for
                   data type\") at the copy-up; podvm does not walk H at
                   the start, as H is often read only. Each other rung and tier refuses it (125), never a
                   writable fallback; `system info --format '{{json
                   .VolumeOverlay}}'` names the rungs (TODO/enter.md T-2101).
                   :mapped (or mapped,ro), the machine tier only: a 9p
                   share with security_model=mapped-xattr; a user of the
                   image writes as its own uid, its owner and mode live in
                   user.virtfs.* attributes, and the host user owns and
                   removes each file (TODO/machine.md T-2103)
  --unsafe-host-paths
                   podvm's own, userland rung only: opt into the
                   pass-through with no PODVM_MAPS anchoring, so absolute
                   guest paths resolve on the host and a write lands on
                   the host tree. Where chroot is denied, a static payload
                   runs on the memfd family only with this flag, and
                   refuses at 125 without it (TODO/enter.md T-1648). The
                   host-root line prints even under -q, and --strict
                   refuses (TODO/enter.md T-1407)
  -u, --user U:G   run as this identity: numeric uid and gid, or names from
                   the image's own passwd and group files. The requested id
                   is answered through the interposer identity memo for a
                   reachable payload, which the banner names and --strict
                   refuses (TODO/interpose.md T-0711)
  --entrypoint P   replace the image's entrypoint.
                   note: As docker: this also
                   drops the image's Cmd, because those were that
                   entrypoint's default arguments
  --add-host N:IP  add a name to the container's /etc/hosts. Repeatable
  --no-source-fixup
                   leave the image's package sources exactly as extracted.
                   note: podvm rewrites http:// to https:// for a mirror that
                   answers over HTTPS, because tcp/80 HANGS on the runtimes
                   podvm targets. This turns that off, and UNDOES a rewrite
                   an earlier container made in the same shared rootfs
  --no-host-cas    do not append this machine's announced CA bundle
                   ($SSL_CERT_FILE, $CURL_CA_BUNDLE, $REQUESTS_CA_BUNDLE) to
                   the image's own trust store.
                   note: Where this machine
                   intercepts TLS, an https package source then fails to
                   verify inside the container, exactly as it does under
                   docker
  --no-steps       refused: do not run any COMMAND inside the rootfs before the
                   payload. Two fixups cannot be made from outside the
                   chroot -- `pacman-key --init` for an empty keyring, and
                   `openssl rehash` for a hash-indexed CA directory, which is
                   the only trust store libzypp reads -- and podvm names
                   each on the banner before it runs it. This refuses them
                   all, and the fixup log then says what the caller gave up
  --strict         refused: refuse to run where a difference of this run
                   is one of the safety classes that podvm can attribute (no
                   path virtualization, host and libc mixing), and warn on
                   each other difference, a gap that podvm cannot attribute
                   too (the raw-syscall escape of the userland rung): the
                   warning names it. --strict=all refuses each one
                   (TODO/cli.md T-1415, T-1831)
  -q, --quiet      force the banner off.
                   note: Runs are already quiet by
                   default; this forces silence under --verbose too
                   (TODO/cli.md T-1416)
  --verbose        print the banner.
                   note: -v is NOT this flag: -v is
                   docker's --volume, so the verbose switch is long-only
                   (TODO/cli.md T-1416)
  --platform P     which platform of a multi-platform image to run
  --pull WHEN      never | missing (default) | always
  --insecure-registry HOST, --tls-verify=B
                   as `podvm pull`; used only when something must be fetched
  -t, --tty        refused: REFUSED BY NAME where /dev/ptmx is unusable, rather
                   than silently degraded (TODO/enter.md T-0503)
  --podvm-tier T  podvm's own: machine selects the machine tier, which needs
                   qemu-system-x86_64 on the host (`podvm doctor` checks
                   it, TODO/machine.md T-1849), incus runs
                   the image in an incusd that answers on INCUS_DIR's
                   socket (run only, TODO/machine.md T-1718), emulate runs
                   it under a static qemu-user of PATH on the native-static
                   rung (run only, TODO/enter.md T-1651), syscall runs a
                   static program on that rung with a fixed uname, a
                   /tmp of 256 MiB (--tmpfs /tmp:size=N sets the cap; a
                   write past it is ENOSPC) and guest paths in
                   /proc/self; run -d and
                   create keep it, and exec in such a container enters it
                   (TODO/enter.md T-1657, T-1935), and each
                   other value is a flag error. With no flag the ladder
                   selects, under every name the binary answers to, and
                   `system info` names the entered rung (TODO/machine.md
                   T-1302, TODO/cli.md T-1712)
  --ulimit N=S[:H] a resource limit of the payload (nofile, nproc, fsize,
                   cpu, as, core, ...), -1 for none; repeatable
                   (TODO/cli.md T-1655)
  --hostname NAME  the host name of the payload (uname, gethostname,
                   /etc/hostname); a UTS namespace where the rung has one
                   (TODO/enter.md T-1908)
  --profile NAME   a TOML recipe of run flags (`podvm profile list`). A
                   profile that names an image takes each positional as the
                   command (TODO/cli.md T-1654)
  --cpus N, --pids-limit N
                   limits of a native run where the host delegates a cgroup2
                   subtree, and of the incus tier; else refused by name.
                   N of --cpus is from 0.01 to 1024 (a whole number is a
                   count of CPUs, a part is a share of time); N of
                   --pids-limit is from 1 to 4194304; a value out of its
                   range is a flag error (TODO/machine.md T-1747, T-1897,
                   TODO/gate.md T-1947)
  --storage-opt size=N
                   the incus tier: the size of the root disk of the
                   instance (a copy of the root device of the default
                   profile); a pool that holds no size (the driver dir)
                   refuses the run (TODO/machine.md T-1964). N is from
                   67108864 to 9223372036854775807 bytes. The machine
                   tier: a disposable ext4 disk of N bytes (64m to 16T)
                   holds the root and the working directory of the guest,
                   and a write past it gets ENOSPC; it does not bound a -v
                   volume, which is a 9p mount of the host. A guest that
                   cannot mount it ends the run with 125 (T-1968). Each
                   other tier refuses it by name
  --network broker, --allow HOST, --ports P
                   an egress broker that allows HOST on the ports P (443 by
                   default); it needs --podvm-tier=syscall or machine
                   (TODO/net.md T-1665)
  --attach S, --expose P, --log-driver json-file
                   accepted and change nothing (docker parity); --strict
                   refuses them (TODO/supervise.md T-0605)
  --security-opt no-new-privileges
                   PR_SET_NO_NEW_PRIVS in the payload; each other option is
                   refused by name (TODO/cli.md T-1655)
  --tmpfs P[:O]    with size=N: a tmpfs in memory of N bytes at P on the
                   namespace rung, for run, run -d and create; a write past
                   N gets ENOSPC, and each other rung refuses it with 125
                   (TODO/machine.md T-1968). With no size=: an empty
                   writable directory at P for a foreground run, removed at
                   its end, not a mount (TODO/cli.md T-1655)
  --read-only      a write outside a writable volume fails with EROFS: on the
                   namespace rung a read-only bind of the root (/tmp, each
                   --tmpfs, each volume and /dev stay writable; T-2108);
                   else through the interposer of the userland loader
                   family; elsewhere it is refused by name (TODO/cli.md
                   T-1655, T-2064)
  --cap-drop ALL   the same bit; a named capability is refused
                   (TODO/cli.md T-1655)
  --timeout S      podman's: at S seconds the payload and its tree are
                   killed with SIGKILL; `.End.Reason` reads wall. S is
                   from 1 to 18446744073709551 (TODO/cli.md T-1655)
  --podvm-qemu-arg A
                   podvm's own, machine tier only and refused elsewhere: one
                   token for the emulator per occurrence. Repeatable, and never
                   split on whitespace (TODO/machine.md T-1302)
  --podvm-mem S   podvm's own, machine tier only and refused elsewhere: the
                   memory of the virtual machine in bytes, with an optional
                   K/M/G/T suffix, from 268435456 (256M, the least that a
                   guest booted with) to 9223372036854775807 bytes, as
                   QEMU's -m; the guest kernel keeps a
                   part of it (256M gives 85 % to MemTotal). A
                   guest over the RLIMIT_FSIZE ceiling is refused before it
                   starts, naming both numbers (TODO/machine.md T-1305)
  --podvm-volume-base DIR
                   podvm's own, the namespace rung and the userland rung
                   (the syscall tier and the loader family): each -v
                   source under DIR opens under it with no symbolic link;
                   the bind or the handler takes that descriptor, and the
                   interposer of the loader family opens it so again at
                   each path call. A link under DIR at the start is a
                   refusal (exit 125). A link in a volume after the start
                   resolves in guest names, as a link of the payload does,
                   so it never reaches a host file out of DIR; a link that
                   the host makes between that walk and the call gives
                   ELOOP. Each guest tier refuses it
                   (PodUtils/podrunner#277, #300)
  --podvm-cpus N  podvm's own, machine tier only and refused elsewhere: the
                   guest's vCPUs, 1 to 64 (TODO/cli.md T-1654)
  --podvm-net none  podvm's own, machine and incus tiers: the guest has
                   no NIC, and `ip link` shows lo only (TODO/machine.md
                   T-1751, T-1779)
  --podvm-channel HOST_SOCKET:127.0.0.1:PORT  podvm's own, machine and
                   incus tiers: each TCP connection to PORT in the guest
                   reaches the Unix socket HOST_SOCKET of the host, over
                   one virtio-serial port, or the socket mounted in the
                   instance (TODO/machine.md T-1751, T-1779)
  --podvm-engine   podvm's own, machine and incus tiers: the guest loads
                   the modules of a container engine (bridge, veth,
                   netfilter, overlayfs) from the pinned modloop
                   (TODO/machine.md T-1780); an incus instance gets
                   security.nesting and the intercepts of mknod and
                   setxattr (T-1966)
  --podvm-init image|podvm
                   podvm's own: image boots the init of the image
                   (/sbin/init, systemd) as PID 1, waits for systemctl
                   is-system-running (180 s, PODVM_INIT_BOOT_TIMEOUT), then runs the command, and
                   ends the init when the command ends. On the incus tier
                   with -d or create (TODO/machine.md T-1993). On a
                   foreground run, the first place that the host gives,
                   with one line that says why: a PID namespace of the
                   namespace rung (a subordinate id range and a delegated
                   cgroup2 subtree); a gVisor sandbox of the runsc tier
                   (runsc and a user namespace); a guest of the machine
                   tier (KVM or TCG). --podvm-tier=runsc or machine names
                   one (TODO/enter.md T-2002, T-2007). With -d or create
                   and no tier, the namespace rung only: exec joins the
                   init, and stop ends the command, then the init; a
                   host with a missing part refuses by name (125). -t
                   gives the command a pty of the init. podvm, the
                   default, keeps podvm's init
  --podvm-vm       podvm's own, incus tier with run -d or create and
                   --podvm-init=image: the instance is a VM that boots
                   the kernel and the initrd of the image on a root disk
                   that podvm writes once for each digest; incusd without
                   VM support refuses by name (TODO/machine.md T-1998)
  --podvm-sealed   podvm's own, machine tier only: the run comes as a tar
                   bundle on stdin (.podvm-run.json with argv, env and
                   workdir, and the files of the guest), and reaches the
                   guest through the channel only: no NIC, no host file of
                   the run (TODO/machine.md T-1751)
  --label K[=V]    a label of the container, repeatable; a repeated key
                   keeps its last value. `ps --filter label=K[=V]` matches
                   it and `inspect` prints .Labels (TODO/cli.md T-1695)
  --report PATH    podvm's own: one JSON document of the run at PATH,
                   written with a rename at its end, also when podvm refuses
                   before the start: the image and its digest, the rung,
                   argv, the exit code or the signal, the times, the CPU
                   time and the peak resident set, the timeout, the limits,
                   the labels and the fixups. Never stdout, which is the
                   payload's, and no value of an environment variable
                   (TODO/cli.md T-1695)
  --podvm-confine SPEC
                   podvm's own: read=P,...;write=P,...;exec=P,...;connect=
                   PORT,... confines the payload with Landlock, the seccomp
                   deny filter of pod-confine and no_new_privs. Each path
                   that no rule names is denied; the system directories of
                   the image are read and execute, /etc is not. Each run
                   may also read /proc, /dev/zero, /dev/random,
                   /dev/urandom and the files of the dynamic loader, and
                   write /dev/null. With no connect=, TCP is not
                   confined (PodUtils/podvm#499). Landlock checks
                   the program that execve names, not one that the
                   dynamic loader runs (ld.so PROGRAM), and it governs no
                   change of mode, owner or times (chmod, chown, utimes):
                   on a rung with a chroot only Landlock holds, so give
                   a -v that must stay as it is :ro. On the userland
                   rung the interposer (the loader family) and the
                   handler (the native-static rung, the syscall tier)
                   answer EACCES to both: an exec of the loader takes
                   the exec= rule of its program (a program with no / is
                   denied), and a change of mode, owner or times needs a
                   write= path. A raw system call of a dynamic program
                   passes the interposer (TODO/enter.md T-1984,
                   T-1985). Each interpreter of a #! script takes the
                   exec= rule on each rung (on the syscall tier the
                   handler, T-2015). On a rung
                   with a chroot and on each family of the userland rung
                   (the loader family, native-static, the syscall and the
                   emulate tier: host paths through the table, -v
                   included; a rule on a path above a -v holds the -v
                   too, as read=/ does, T-2014); the record keeps it, so
                   -d and each exec take it. The supervise rung, the machine tier, and a
                   kernel with no Landlock refuse by name (TODO/enter.md
                   T-1658)

  refused: podvm run enters a CHROOT, not a container. It shares this machine's
    process table, network, IPC and mount namespaces with the payload. With
    --verbose, the banner on stderr says so and names what the selected rung
    must never claim; a default run is quiet.

  note: The payload owns stdout. Every word podvm prints goes to stderr, so
    `podvm run <image> cmd | consumer` gives the consumer the payload's
    bytes and nothing else.

  note: The exit code is the payload's own, and a signalled payload is 128+signal.
    125 is podvm failing to run the command, 126 the command found and not
    invocable, 127 not found. Those are docker's.
";

mod parse;

/// The parser of `run` and `create` for the table of [`crate::number`].
#[cfg(test)]
pub(crate) fn parse_probe(verb: &str, args: &[String]) -> Result<(), i32> {
    parse::parse(verb, args).map(drop)
}
pub use parse::*;

mod foreground;
/// `run --report PATH` (TODO/cli.md T-1695).
pub(crate) mod report;
pub use foreground::*;

mod prepare;
// TODO/image.md T-1781: the spare tree of an image.
pub(crate) mod spare;
// TODO/probe.md T-1746: the findings from the cache of T-0111.
pub(crate) mod probe_cache;
pub(crate) use prepare::*;

mod home; // TODO/enter.md T-1986
mod tree;
pub(crate) use tree::drop_unrecorded;

/// The image's entrypoint and `--entrypoint` for `Plan::argv_for`.
/// docker's `--entrypoint ""` clears the image's entrypoint: the command
/// (or the image's `Cmd`) runs alone (TODO/cli.md T-1661).
fn entrypoints<'a>(
    image: Option<&'a [String]>,
    o: &'a Opts,
) -> (Option<&'a [String]>, Option<&'a str>) {
    match o.entrypoint.as_deref() {
        Some("") => (None, None),
        // PodUtils/podrunner#325 (TODO/machine.md T-1993): with
        // `--podvm-init=image` the init of the image boots as PID 1 and
        // stands for its Entrypoint (an image made for an init declares
        // `/sbin/init`). The command runs beside it with no such prefix:
        // before, the instance ran `/sbin/init CMD...` as a second process,
        // which printed "Too many arguments." and ran nothing.
        None if o.init_image => (None, None),
        given => (image, given),
    }
}

#[cfg(test)]
#[test]
fn the_entrypoint_of_the_image_is_the_init_with_podvm_init_image() {
    let image = ["/sbin/init".to_string()];
    for (init_image, given, want_image, want_given) in [
        (false, None, true, None),
        (true, None, false, None),
        (false, Some(""), false, None),
        (true, Some(""), false, None),
        (true, Some("/bin/x"), true, Some("/bin/x")),
    ] {
        let mut o = Opts::new();
        o.init_image = init_image;
        o.entrypoint = given.map(str::to_string);
        let (img, g) = entrypoints(Some(&image), &o);
        assert_eq!(img.is_some(), want_image, "{init_image} {given:?}");
        assert_eq!(g, want_given, "{init_image} {given:?}");
    }
}

/// The refusal of a run with no command. With `--podvm-init=image` the
/// Entrypoint of the image is its init, never the command
/// (PodUtils/podrunner#325).
fn nothing_to_run(verb: &str, image: &str, init_image: bool) -> String {
    if init_image {
        format!(
            "podvm {verb}: --podvm-init=image boots the Entrypoint of {image} as its init, \
             and the image declares no Cmd and no command was given. There is nothing to \
             run beside the init"
        )
    } else {
        format!(
            "podvm {verb}: {image} declares neither an Entrypoint nor a Cmd, and no \
             command was given. There is nothing to run"
        )
    }
}

/// The command under the profile's `fs.wrap` (TODO/cli.md T-1654) or its
/// runtime (T-1661). An empty command stays empty, so that its refusal
/// names the image.
fn wrapped(o: &Opts, argv: Vec<String>) -> Vec<String> {
    crate::profile::runtime::under(&o.wrap, &o.drops, argv)
}

mod acquire;
pub(crate) mod binds; // TODO/enter.md T-1920
pub(crate) mod cgroup; // TODO/cli.md T-1747
mod devices; // TODO/enter.md T-1939
mod fallback; // TODO/enter.md T-1953
pub(crate) mod overlay; // TODO/enter.md T-2101
pub(crate) mod read_only; // TODO/cli.md T-2108
pub(crate) mod tmpfs; // TODO/machine.md T-1968
mod user_ns; // TODO/enter.md T-1927
pub(crate) use acquire::*;

mod wall;

// TODO/cli.md T-1760: the pty of `run -t`.
pub(crate) mod pty;

// TODO/machine.md T-1656: a guest from an OCI image.
mod foreign;
mod incus_tree; // TODO/machine.md T-2085
mod machine;
// TODO/enter.md T-1729: gVisor's runsc.
mod runsc;
pub(crate) use runsc::unserved as runsc_unserved; // T-2007

// TODO/cli.md T-1706: the reason of a SIGXFSZ death.
mod exit_note;

#[cfg(test)]
mod messages_tests;
#[cfg(test)]
mod timeout_tests;

#[cfg(test)]
mod profile_tests;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tier_options_tests; // TODO/gate.md T-1854
