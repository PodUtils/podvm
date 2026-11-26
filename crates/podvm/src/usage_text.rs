//! The usage of `podvm` and of `podvm probe` (PodUtils/podvm#520: the list
//! named no `port` and no `logout`). `tests/cli_contract.rs` checks that each
//! verb of the parity table is in [`USAGE`].

pub const USAGE: &str = "\
usage: podvm <command> [options]

  run          extract an image if needed and run a command inside it
  exec         run a command in an already extracted image. refused: A FRESH CHROOT,
               sharing only the filesystem, never a namespace entry
  probe        report what this machine permits, and the rung podvm selects
  remote       verbs that act across a machine boundary: ssh into another one
  doctor       check what this machine can run, with one fix line per missing piece
  windows      a disposable Windows guest: doctor, fetch, setup and run
  machine      a guest podvm runs: ssh into it (podman parity)
  man          render this manual from the binary's own usage strings
  profile      list and show the profiles that `run --profile` takes
  notebook     run cycles of a notebook that keep only one directory
  lab          plan a network lab in one guest: scenarios, steps, verdicts
  probe-inside run podvm's probe inside an image and write what it gets there
  compare      compare a report of probe-inside with a reference
  pull         fetch an image into the content-addressed store
  save         write one image as an OCI-layout tarball
  load         read an OCI-layout tarball into the store
  import       build an image record from a plain rootfs tar
  extract      unpack a pulled image's layers into a rootfs
  images       list what the store holds
  rmi          remove images, and every blob no image reaches
  tag          point a second name at one manifest digest
  image        ls | rm | prune | tag
  inspect      print one image's record
  verify       hash the store's blobs against their digests
  create       write a container record, and start nothing
  start        start a created container, and return when its payload is running
  update       change resource limits of a running Incus container
  volume rm    remove a volume's host directory and what Incus instances wrote in it
  ps           list containers, from podvm's own table and never from /proc
  logs         a container's captured output
  stop         SIGTERM, then SIGKILL after a bounded grace
  restart      stop, then start, naming which half failed
  kill         send one signal to a container's payload
  wait         block until a container ends, and print its exit code
  rm           remove a container
  cp           copy one file into or out of a container's rootfs
  port         the published ports of a detached Windows machine
  system info  what this podvm is, and the verb and flag parity table AS DATA
  login        log in to a registry (T-1832)
  logout       remove the login of a registry
  container    ls | rm | prune | start | stop and the other container verbs
  system install-names  install `docker` and `podman` as links to podvm
  version      print the version

  Every other docker verb is named in that table with the reason podvm does
  not have it, and says so with exit 125 rather than being silently ignored.
  A verb neither podvm nor docker has exits 1, which is docker's own code for
  it (TODO/cli.md T-0802).
";

pub const PROBE_USAGE: &str = "\
usage: podvm probe [--json | --rows] [--strict] [--cached]

  (no flag)   print the selected rung on stdout and the evidence on stderr
  --json      print the findings as one JSON document on stdout
  --rows      print every probe row on stdout, in the format
              verification/probe emits, so two runs diff line by line
  --strict    exit non-zero when the selected rung is below `namespace`, or
              when a control of the bogus-argument discriminator could not
              answer. A caller gates on this without parsing anything.
  --cached    the hot path: serve $store/probe.json when its key still holds,
              and measure and rewrite it when it does not. Without this flag
              podvm probe measures and touches no store, because a diagnostic
              that can answer from a file is not a diagnostic.

  --probe-child <name>
              internal. Runs one named probe in this process and reports it on
              stdout and in the exit status. podvm re-executes itself with
              this for every probe, because a successful unshare, chroot or
              setuid mutates the prober.
";
