Area: net.

Record semantics: [task rules](RULES.md#5-entry-closure).


This page holds the network work of the widened scope: network labs,
tunnels and live uplinks, packet capture and counters, and the egress
broker. The owner ruled on 2026-11-06 that podvm does what sand-net did,
without root. Issue 193 is the index of that scope.

No other area owns the subject. [machine](machine.md) owns the tier
that hosts a lab, and [cli](cli.md) owns the verbs. The labs, the
broker and the capture are a network subject on both tiers, and the
podcrates crates that serve them (`pod-netlab`, `pod-pcap`, `pod-egress`,
`pod-broker`, `pod-netstack`) are network crates.

The target host has no network namespace, no `/dev/net/tun`, no root and
no `CAP_NET_RAW`. Its only way out is an HTTP CONNECT proxy. A lab
therefore runs inside one machine-tier guest, where the guest kernel is
root, and a rootfs-tier payload shares the host network.

The current work order is [PROGRESS.md](PROGRESS.md).

# T-1662: Network labs: NAT, IPv6, policy and impairment labs inside one guest

**Source:** issue 178; #193; the owner's ruling of 2026-11-06
**Category:** net
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** partial

## Problem

sand-net built 39 lab scenarios from three network namespaces
and checked each with a self-check that derived every expected
verdict from the lab's settings. The target host has no
network namespace and no `/dev/net/tun`, so podvm cannot build
a lab on the host.

## Premise

A machine-tier guest kernel is root inside, so namespaces,
nftables, `tc netem`, tayga and WireGuard work there. The
machine tier runs under TCG on the target host
(`crates/podvm/src/tier.rs:152`), at 3x to 21x the native cost
(T-1308). User-mode networking carried a guest in
`experiments/361-guest-usernet.sh:299`. Running a guest from
`run` is T-1656. The issue names `pod-dns` and `pod-proxy`;
podcrates names `pod-netlab` (wave M7) on `pod-netstack`.

## Approach

Add `podvm lab up|down|check|try|matrix` that boots one guest
with a lab image and kernel options (netns, nf_tables, netem,
wireguard, tun). Port the lab layout and the knobs unchanged.
The guest only measures; the host derives each expected
verdict from the lab's settings and compares, and reads each
nftables rule counter before and after. Refuse contradictory
knobs. For a policy-only lab, offer a lighter variant on a
user-space stack with no kernel, from podnet's `podnet net`.

## Decision

The check design ports as it is: measure in the guest, derive
on the host, read counters, plant a failure. A rootfs-tier
payload as the lab's guest is measured or refused with the
reason. Take `pod-netlab` and `pod-netstack` when podcrates
releases them; the world's DNS and proxy come from podcrates
crates or the entry records why not.

## Prove

`cargo test -p podvm` exits 0 with tests that derive the
expected verdicts of two scenarios from their knobs.
`sh experiments/418-net-lab.sh` exits 0 on a TCG-only host:
`podvm lab try dual-nat` prints PASS rows, `podvm lab matrix`
runs every offline scenario with its time, and a flushed
policy chain inside the guest makes the check fail.

## Correction

**Filed 2026-11-06.** Quoted from issue 178:

```text
namespaces sn-NAME-g|r|w: guest 10.77.0.2 fd00:77::2; router 10.77.0.1,
  fd00:77::1 and 198.51.100.1, 2001:db8:100::1; world 198.51.100.10
knobs: --stack dual|v4|v6, --nat none|nat44|nat66|both, --policy
  open|web|web-nodns|only443|blackout, --broken, --mtu 1280..1499,
  --icmp drop, --delay/--loss/--rate (netem), --dnat, --hairpin, --nat64,
  --cgnat
verdicts: ALLOW, DROPPED, REFUSED, DENY, UNRESOLVED, TIMEOUT
L1: the kernel rate-limits ICMP errors; reject read as silent drops until
  icmp_ratelimit=0
L2: an unconnected UDP socket never sees port-unreachable
L5: `ct state new ct status != dnat drop` dropped forwarded DNAT flows
F18: tayga with 64:ff9b::/96 refused non-global IPv4 (RFC 6052);
  2001:db8:64::/96 works
F20: ping_group_range must be one contiguous gid range
F21: a wildcard-bound UDP server answers from the route's preferred
  address; reply with IPV6_PKTINFO
```

The effort of T-1662 was XL before call C17 of `docs/decisions.md`; pod-todo has no XL.

**Blocked 2026-11-11 on podcrates.** The Decision takes `pod-netlab`,
which podcrates lists under M7 after a study, with no date. COORD-041
(issue 260) asks for it, and asks who reads the guest's modules. Measured
here: the pinned guest kernel (`6.12.110-0-virt`) has `NET_NS` and
`IPV6` built in, and nf_tables, nft_nat, sch_netem, wireguard, tun, veth
and nf_nat as modules; they are in `boot/modloop-virt` (a squashfs with
xz, 21 MB), and `squashfs.ko` is in the modloop only, so the host must
read it and give the guest a module set with its `modules.dep`. The
condition that clears the block: podcrates's answer on the crate and on
the reader. T-1663 and T-1664 run in this lab guest.

**Unblocked 2026-11-13.** podcrates answered on the crate and on the
reader. The reader of the modloop is `pod_guest::modloop::modules`
(podcrates@T-206, in `v0.24.0`). podvm takes `v0.34.0`, which holds the
offline part of `pod-netlab` (podcrates@T-207, `v0.33.0`,
PodUtils/podcrates#103): the knobs, the layout and the scenarios of issue
178, the plan of a lab, the derivation of each verdict and the judge of
the counters. podvm can now build `podvm lab` on the host and the first
clause of the Prove (the derived verdicts of two scenarios). The probe
program in the guest (podcrates@T-345), the world (podcrates@T-346) and
the run in the guest (podcrates@T-347) follow in podcrates; experiment
418 waits for them.

## Done

**Partial 2026-11-13.** The host side of `podvm lab` on the offline part
of `pod-netlab` (`v0.34.0`), in `crates/podvm/src/lab.rs`: `podvm lab
scenarios` lists the 27 offline scenarios with their flags, and `podvm lab
plan SCENARIO` (or the knob flags of sand-net) checks the knobs and prints
the steps that build and remove the lab in the guest (namespaces
`pv-NAME-g|r|w`), the verdict of each probe and the counters that must
move or stay. Each knob flag has a row of the parity table; a bad knob or
a contradiction exits 125 by name. `up`, `down`, `check`, `try` and
`matrix` exit 125 and name the guest parts that they wait for. The first
clause of the Prove holds: `cargo test -p podvm` exits 0 with
`the_verdicts_of_two_scenarios_derive_from_their_knobs` (`dual-nat`:
`tcp4-443` ALLOW and the counter `return` moves; `blackout`: `tcp4-443`
DROPPED, `return` stays and `dropped` moves) and five more tests.
Remaining: the guest with the lab kernel options and the module set of
the modloop, the run of the steps and the probes in it, the judge of the
counters, and experiment 418 (`lab try dual-nat`, `lab matrix`, a flushed
chain fails the check); they wait for podcrates@T-345 to podcrates@T-347.

# T-1663: Tunnels and live uplinks: WireGuard kill switch, WARP uplink, proxy-forced programs

**Source:** issue 180; #193; the owner's ruling of 2026-11-06;
  A-V-02 of PodUtils/podcrates#39
**Category:** net
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** blocked

## Problem

sand-net ran WireGuard labs with a kill switch, a live WARP
uplink, and a mode that forced a proxy-ignorant program
through a proxy. podvm has none of the three, and the target
host has no kernel facility for the first two.

## Premise

WireGuard and the kill switch need a kernel, which the lab
guest of T-1662 has. podnet's `warp` needs no TUN in its proxy
and `run` modes; its TUN mode needs `CAP_NET_ADMIN`, which the
guest has. podnet's `pod` (crate `podnet-netns`, seccomp
backend) forces a program's traffic through a proxy with no
root. The issue names `pod-proxy` and `pod-tls`.

## Approach

Run the vpn scenarios inside the T-1662 guest unchanged. Run
podnet inside the guest as the router's uplink, its outer
connection through user-mode networking and the host proxy.
On the rootfs tiers, add a net mode that reuses
`podnet-netns` to force a payload through a proxy URL. For a
kill switch without a namespace, refuse in a Syscall User
Dispatch handler (T-1652) every `connect` but the proxy.

## Decision

podvm reuses podnet's crate and does not write a second one. A
kill switch on a rootfs tier is per payload, not per host, and
the output says so. A dead upstream fails the connection
visibly, not by a reset after a successful `connect` (cfrs
issue 190).

## Prove

`cargo test -p podvm` exits 0 with tests for the net mode's
argument parse. `sh experiments/419-net-tunnels.sh` exits 0:
vpn-full, vpn-leak and vpn-killswitch pass in the lab guest with
the `leak` counter moving only in the kill-switch case, a
proxy-ignorant program reaches the world through the proxy
mode, and a dead upstream fails that program's connection
visibly. The live warp-uplink leg runs where the operator
grants a live run.

## Blocker

**Blocked 2026-11-11 with T-1662.** Each clause of the Prove runs in the
lab guest of T-1662, which waits for `pod-guest`'s reader of the modloop
(podcrates podcrates@T-206) and `pod-netlab` (podcrates podcrates@T-207); podcrates answered
COORD-041 (issue 260) on PodUtils/podcrates#70 that this entry waits with
it. The parts with no lab are in place: the kill switch of a rootfs-tier
payload is the hold of T-1665 (the handler refuses each `connect` but the
broker's).

## Correction

**Filed 2026-11-06.** Quoted from issue 180:

```text
--vpn full|split: tunnel 10.99.0.0/24 and fd99::/64, port 51820,
  persistent-keepalive 1; full routes 0.0.0.0/1, 128.0.0.0/1, ::/1, 8000::/1
--killswitch: accept lo, wg0, IPv6 ND and UDP 51820 to the endpoint; drop the
  rest with a counter named `leak`
F22: an nftables drop in OUTPUT is silent for TCP and EPERM for UDP and
  ping sockets, so a kill switch reads DROPPED for TCP and DENY for UDP
F23: `cfrs warp tun` kept working after its TUN moved to another namespace
cfrs 0.3.0-beta.8 on a proxy-only host: `cfrs doctor` 2 of 4 edge legs;
  in the warp-uplink lab 4 of 4; warp-v6-only 2 of 4
quick-tunnel provisioning hit 429 with Retry-After: 1379 on the eighth
  provision in about 70 minutes
```

**Amended 2026-11-09.** The ask A-V-02 of PodUtils/podcrates#39 (COORD-015,
a message from a peer session) wants a payload held to one egress. The hold
is the shared mechanism of this entry and of T-1665: a Syscall User Dispatch
handler (T-1652) or podnet's `pod`. podcrates podcrates@T-109 studies the hold of
podnet. T-1665 holds the clauses of the ask. This entry changes no clause.

# T-1664: Observability: capture, counters and connection logs without `CAP_NET_RAW`

**Source:** issue 182; #193; the owner's ruling of 2026-11-06
**Category:** net
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** blocked

## Problem

sand-net captured packets, read per-interface and rule
counters, and watched rates inside a lab, all through `sudo`.
On the target host `AF_PACKET` needs `CAP_NET_RAW`, which an
unprivileged process does not have. A rootfs-tier payload
shares the host network, so nothing can capture its packets.

## Premise

Inside the T-1662 guest the kernel is root, so capture and
counters run there. QEMU's `filter-dump` writes a pcap of a
guest's user-mode network with no privilege. The interposer
and the T-1652 handler see each `connect`, `sendto` and `bind`
call. The issue names podcrates `pod-pcap` (wave M7) through
the podcrates crate table.

## Approach

Add `podvm lab capture|stats|watch` that runs the capture and
the counters in the guest and streams them out on a second
channel. For a machine-tier run with user-mode networking,
write a host-side pcap through `filter-dump`. For a rootfs-tier
payload, write a connection log at the syscall layer: address,
port, result and errno for each call.

## Decision

A connection log is not a packet capture, and its output says
so. The counters are part of correctness: the lab check fails
when a rule that should match did not. The proxy variables of
the calling shell are removed in a lab shell, because they
name a host loopback that the lab does not have. Take
`pod-pcap` when podcrates releases it.

## Prove

`cargo test -p podvm` exits 0 with a test for the connection
log record format. `sh experiments/420-net-capture.sh` exits 0:
`podvm lab capture LAB --at router --iface rw -w out.pcap`
writes a pcap that `tcpdump -r` reads and that shows the NATed
source, `podvm lab stats LAB --json` lists the rule counters by
name, a machine-tier run writes a host-side pcap with no
privilege, and a rootfs-tier run logs each connect with its
errno.

## Blocker

**Blocked 2026-11-11 with T-1662.** Each clause of the Prove runs in the
lab guest of T-1662, which waits for `pod-guest`'s reader of the modloop
(podcrates podcrates@T-206) and `pod-netlab` (podcrates podcrates@T-207); podcrates answered
COORD-041 (issue 260) on PodUtils/podcrates#70 that this entry waits with
it. The host-side pcap and the connection log at the syscall layer need no
lab and come with the entry.

## Correction

**Filed 2026-11-06.** Quoted from issue 182:

```text
sand net capture LAB [--at guest|router|world|carrier] [--iface IF] [-w FILE]
  [--seconds N] [-c N] [-q]: AF_PACKET with ETH_P_ALL; Ethernet keeps link
  type 1, wg0 is raw IP (type 101); the file is chowned to the caller
sand net stats: ip -j -s link, tc -s -j qdisc, nf_conntrack_count, and the
  router's nft counters return, open, tcp, udp, dropped, broken, inbound,
  forwarded, ping
in dual-nat, 10.77.0.2 leaves rw as 198.51.100.1
F7: `ip netns exec` remounts /sys; a capture helper is fine, runc is not
not covered: capture inside a cage's own namespace
```

# T-1665: An egress broker, and the guest's view of a loopback-only proxy

**Source:** issue 184; #193; the owner's ruling of 2026-11-06;
  A-V-02, A-V-03 of PodUtils/podcrates#39
**Category:** net
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** done

## Problem

The target host's only way out is an HTTP CONNECT proxy, often
on the host loopback. A machine-tier guest cannot reach a
loopback proxy without a rewrite. podvm has no egress broker
that holds a payload to an allowlist, as errand's broker and
sand-net's replica did.

## Premise

A rootfs-tier payload shares the host network, so a loopback
proxy is reachable and the variables pass through unchanged.
QEMU user-mode networking maps the host loopback at 10.0.2.2
unless `restrict=on`; the usernet experiment routes through
that gateway (`experiments/361-guest-usernet.sh:223`). The
tree reads no proxy variable today. The issue names podcrates
`pod-proxy` and `pod-tls`; podcrates names `pod-broker` for
the broker (podcrates podcrates@T-099, podcrates@T-100, call C25). `pod-egress` keeps the
roads (podcrates podcrates@T-035).

## Approach

On the machine tier, rewrite each proxy variable whose host is
`127.0.0.1`, `localhost` or `::1` to 10.0.2.2, scheme, port and
credentials byte for byte. Add `--net broker` with `--ports`
and `--allow`, built on `pod-broker` and `pod-proxy`, chained
to the host proxy. Hold a payload to the broker with a Syscall
User Dispatch handler that refuses every other `connect`
(T-1652), or with podnet's `pod`.

## Decision

The broker's 407, 403, 502 and 504 vocabulary of issue 184 is
the specification. A profile never claims `broker` where no
holding mechanism is active. A seccomp filter cannot read a
sockaddr, so it is not a holding mechanism. The credential
never goes in argv or in a URL that podvm logs.

## Prove

`cargo test -p podvm` exits 0 with rewrite tests: a password
with `@` survives, a percent-escaped password is not decoded,
a URL with no port or no scheme is refused.
`sh experiments/421-egress-broker.sh` exits 0 on a
loopback-proxy host: a rootfs-tier payload fetches through the
unchanged variables, a machine-tier guest gets the proxy at
the gateway with credentials intact, `--net broker --ports 443`
answers 403 for port 22 and for 127.0.0.1, 407 without the
session credential and 200 for an allowed host, and a payload
that ignores the variables cannot reach the network directly. A `connect` to a named upstream
reaches the broker on its Unix socket, the name resolves to a fake
address, and with no holding mechanism the run refuses by name.

## Correction

**Filed 2026-11-06.** Quoted from issue 184:

```text
broker: Basic Proxy-Authorization with a per-run 32-byte hex secret, else
  407; port or host off the lists: 403 not on the egress allowlist; name
  not resolved in 5 s: 504; not a public host (loopback, private,
  link-local, reserved, 168.63.129.16): 403; dial over 10 s: 502
F29: 127.0.0.1 in the guest is the guest; slirp4netns reaches the host
  loopback only with allow_host_loopback=true, at 10.0.2.2:
  `curl -x http://10.0.2.2:3128 https://api.github.com` gave 407; without it
  the connection was refused
F27: podman 5 copies the host proxy variables into every container
  (--http-proxy defaults to true), so a guest failed naming itself
F11: the reference report showed CONNECT answering 200 with no credential,
  while errand's document says 407
busybox wget sent plain HTTP to the proxy and got 404
```

**Amended 2026-11-09.** The asks A-V-02 and A-V-03 of PodUtils/podcrates#39
(COORD-015, a message from a peer session) and the call C25 of podcrates
change this entry. C25: the broker is the new crate `pod-broker` (podcrates
podcrates@T-099, podcrates@T-100), not `pod-egress`. The Premise and the Approach now name
`pod-broker`, and `--net broker` uses it. A-V-03 wants the policy of the
egress proxy of cfb. That policy is an input of `pod-broker`: the allowed
hosts and ports, the per-run credential, the rule for public hosts and the
407, 403, 502 and 504 answers. `--net broker` loads it and does not write a
second copy. A-V-02 wants a payload held to one egress. Each `connect` to a
named upstream goes to the broker on a Unix socket. A fake DNS answers each
name with one stable fake address, and the hold maps that address to the
upstream. When no holding mechanism works, the run refuses by name before
the start. The Decision already says this.

**Amended 2026-11-10, for podrunner.** The podrunner design review (the
owner's design review of PodUtils/podcrates#47; the ruling "podrunner",
2026-11-10; SPEC sections 9.1, 9.5 and its unknown U9) holds a podvm unit with
`podnet pod` around `podvm run`, and calls that hold advisory. podnet owns
the measurement of the pod with the runner (COORD-022, issue 228). One
clause is podvm's: the experiment adds a run of `podnet pod
--default-action reject -x http://unix:/PATH -- podvm run IMAGE`, where
the payload's TCP reaches the Unix upstream, a direct connect is refused,
podvm's own pull before the payload works or refuses by name, and the
banner names the rung. The review writes the upstream as
`socks5://unix:PATH`; `pod-broker` speaks HTTP CONNECT only, and podnet
parses `http://unix:/PATH` (COORD-021, issue 227, section 0, item 4).
Under the seccomp listener of the pod the kernel refuses a second
listener, so the probe rejects `supervise` there (Inferred, not run); the
experiment records the rung that it selects. The report of T-1695 names an
outer hold only where podvm measures it, else `null`.

## Done

**Done 2026-11-11.** The last clause: a payload that ignores the
variables reaches a named upstream through the broker. With `--network
broker` on the syscall tier, podvm adds each exact name of `--allow` to
the container's `/etc/hosts` with a fake address `198.18.0.N`
(`crates/podvm/src/broker.rs`, `fake_hosts`), and gives the handler the
credential and the names in `PODVM_DISPATCH_BROKER`, which passes each
`execve`. The handler turns a `connect` to `198.18.0.N:PORT` into a
connect to the broker on the loopback, sends `CONNECT NAME:PORT` with the
Basic credential of the run, and reads the head one byte at a time before
the call returns: 200 gives 0, 403 and 407 give EACCES, each other answer
ECONNREFUSED (`crates/podvm-dispatch/src/hold/redirect.rs`, no
allocation). The broker applies its policy as for each other client. A
pattern or an address of `--allow` gets no fake address: the broker would
refuse its names anyway. Experiment 421 clause 8 (exit 0, with
`PODVM_421_MIRROR=1`, on the cloud host): `/etc/hosts` reads
`198.18.0.1 example.com`, `http_proxy= wget http://example.com/` gets
`<title>Example Domain</title>`, and port 22 of the same name fails with
"Permission denied" from the broker's 403. `cargo test -p podvm-dispatch`
and `cargo test -p podvm --bin podvm broker` exit 0 with the tests of the
fake address, the request and its credential, base64 and the head.

**Partial 2026-11-11.** The rewrite half is done. `cargo test -p podvm`
exits 0 with `a_loopback_proxy_moves_to_the_gateway_byte_for_byte` (a
password with `@` survives, a percent escape is not decoded) and
`a_url_with_no_port_or_no_scheme_is_refused_without_its_value`
(`crates/podvm/src/machine/proxy.rs`). On the machine tier, each proxy
variable of the payload whose host is `127.0.0.1`, `localhost` or `::1`
names the gateway 10.0.2.2, and a proxy URL with no scheme or no port
refuses the run, naming the variable and never its value.
`PODVM_421_MIRROR=1 sh experiments/421-egress-broker.sh` exits 0
(`experiments/results/egress-broker.txt`): with a loopback proxy of the
script that wants a Basic credential with an `@` in its password, a
rootfs-tier payload fetches through the unchanged variable (200), a TCG
guest sees `http://CRED@10.0.2.2:PORT` and fetches (200), the proxy
answers 407 without the credential, and a URL with no port refuses the
guest by name with no password in the message.

Measured beside it: the guest reached this host's own loopback proxy at
10.0.2.2, and its HTTPS through that proxy failed the certificate check,
because that proxy presents its own CA and the guest has no host CA
completion (the completion of T-0407 runs on the rootfs tiers).

The broker half, 2026-11-11: `run --network broker --ports P --allow HOST`
hosts `pod-broker` for the run (`crates/podvm/src/broker.rs`): one job with
a token of 32 random bytes, a policy that allows each host of `--allow` on
the ports of `--ports` (443 by default), and the host's proxy as the way
out (the target's name goes to it). The payload's proxy variables name the
broker with the token as the password; the token is in no argv and in no
line that podvm prints. Two holding mechanisms:

* the syscall tier (T-1657): the handler lets `connect` reach the broker's
  port on the loopback and Unix sockets only, and refuses a datagram with
  an address and `io_uring_setup`, with EACCES
  (`crates/podvm-dispatch/src/hold.rs`, the test
  `the_hold_lets_the_payload_reach_the_broker_only`);
* the machine tier: the guest network is `restrict=on`, and its one way
  out is `guestfwd` at 10.0.2.100:3128 to a relay of podvm for each
  connection (`--podvm-relay`; a `tcp:` chardev is one connection for the
  whole run, and the first try hung on it).

Each other rung refuses `--network broker` by name: a payload that ignores
the variables would reach the network there. A profile takes `net.mode =
"broker"`, `net.ports` and `net.allow`. Experiment 421 clauses 5 to 7 (exit
0): on the syscall tier (the static busybox, on the fixture) and in a TCG
guest, the allowed host answers 200, port 22 and 127.0.0.1 answer 403, no
credential answers 407, and a direct connect fails (EACCES; refused in the
guest); with no holding mechanism the run refuses (exit 125). busybox:musl
has no TLS client, so the clauses allow port 80 beside 443 and fetch the
allowed host over plain HTTP.

Remaining clause: a `connect` of a payload that ignores the variables to a
named upstream reaches the broker (its Unix socket), the name resolving
to a fake address that maps to the name. Today such a payload is refused,
not served.

# T-1770: `podvm lab`: a knob line with an empty value, and the words of the errors of the knobs

**Source:** experiment 443 (`experiments/443-qa-surface.sh`, the QA pass of 2026-11-13, [the run](../experiments/results/qa-surface.txt))
**Category:** net
**Milestone:** none
**Priority:** P3
**Effort:** S
**Status:** done

## Problem

`lab scenarios` prints `--udp ` with no value for corp-proxy, and that line does not parse back; `--mtu 99999` says "not a whole number"; `--nat bogus` does not list the words.

## Premise

`Knobs::flags` and the errors of the knobs are `pod-netlab`'s.

## Approach

A COORD issue to podcrates for the flags and the words; podvm takes the tag.

## Decision

The fix in the lab is podvm's; a cause in a crate of podcrates goes there as a COORD issue.

## Prove

`sh experiments/443-qa-surface.sh` gives PASS for the rows `lab.knobs.corp-proxy`, `lab.mtu-big`, `lab.nat-bad`, and a test of the lab holds the fix

## Done

**Done 2026-11-13.** podcrates `v0.38.0` (PodUtils/podcrates#136, its
podcrates@T-503) fixes the three in `pod-netlab`, and podvm takes the tag
with no change of its own. On the cloud host: `lab scenarios` prints
`corp-proxy ... --nat both --tcp 3128`, which `lab plan` takes at exit 0;
`lab plan --nat both --mtu 99999` says `the value is out of its range 1280
to 1499`; `lab plan --nat bogus` says `the value is not one of: none,
nat44, nat66, both`.


# T-1820: An allowed host through `--network broker` answers 502 "the far end failed"

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), O-04 (MACH-08; tester-reported, not re-run by the report owner)
**Category:** net
**Milestone:** none
**Priority:** P1
**Effort:** M
**Status:** done

## Problem

On the machine tier, `--network broker --allow example.com` gives
`HTTP/1.1 502 Bad Gateway: the far end failed` for `CONNECT
example.com:443` and for `www.example.com` under `*.example.com`, in four
runs on ports 80 and 443. The policy answers are right (403, 407,
loopback). The host reaches the same URLs through the same proxy with
200. T-1665 promises 200 for an allowed host.

## Premise

Not established. The broker dials the far end through the inherited
proxy of the host, and prints no upstream status. The cause is in
`pod-broker` (podcrates) or in podvm's use of it
(`crates/podvm/src/broker.rs`), or in the proxy of the QA host.

## Approach

Print the upstream status line and the proxy that the broker used, with
no body and no credential, under `--verbose`. Add a fixture proxy that
answers 200 to the exact CONNECT that the broker sends, and a test
through it. If the CONNECT form is wrong, podvm asks podcrates (proposed
COORD issue).

## Decision

A 502 names its cause.

## Prove

`cargo test -p podvm --test broker_upstream` exits 0, with a test that an allowed host behind a fixture proxy answers 200, and that a refused upstream prints its status line


## Correction

**2026-11-14.** The Prove names `cargo test -p podvm --test
broker_upstream`. The broker of podvm is a module of the binary, so the
test is a test of the binary: `cargo test -p podvm --bin podvm
broker_upstream`.
## Done

**Done 2026-11-14.** `broker::start_with` takes the upstream proxy as a
value (`start` reads it from the environment as before), and
`broker/upstream_tests.rs` runs the broker against a fixture upstream on
the loopback. `cargo test -p podvm --bin podvm broker_upstream` exits 0:
with an upstream that answers 200, a `CONNECT example.com:443` with the
broker's credential gets `HTTP/1.1 200`, and the upstream saw `CONNECT
example.com:443 HTTP/1.1`; with an upstream that answers 403, the client
gets `HTTP/1.1 502` and the text holds `403` (podcrates `v0.45.0`,
podcrates@T-609). The 502 of the QA host stays a cause of that host's
proxy, which this text now names.

**Partial 2026-11-14.** Measured on the cloud host, which egresses
through its own proxy: a machine guest with `--network broker --allow
example.com` sends `CONNECT example.com:443` with the broker's
credential and gets `200 Connection Established`; the 502 of the QA host
does not reproduce here, and its upstream proxy is the likely cause. Two
gaps of the text stay, both in `pod-broker`: a 502 names no upstream
status, and a `GET http://example.com/` gets `403 ... no-rule` with no
word that the default ports are `[443]` and `--ports 80` is the fix.
busybox `wget` sends `GET https://...` to a proxy and gets 400 from the
broker; that is the client's form, and a client that sends `CONNECT`
works. The clause that remains: the texts, which podcrates `v0.45.0`
carries (COORD-108, issue 359, podcrates@T-609), and the test
`broker_upstream` of the Prove with them.

# T-1847: The scenarios and knobs of `lab` differ from the surface that sand-net documents

**Source:** the external QA report of `v0.2.0-beta.14` (2026-11-14), MISC-03 (a gap of scope, not a regression)
**Category:** net
**Milestone:** none
**Priority:** P3
**Effort:** M
**Status:** done

## Problem

`podvm lab scenarios` lists 27 scenarios, and sand-net lists 39: the
`corp-proxy-pod`, `vpn-*` and `warp-*` scenarios are absent. Five value
sets differ (`symmetric-nat`, `pmtu-*` with MTU 1400 against 1280,
`cgnat*`, `nat64-*` with NAT66 on, `locked-443` and `corp-proxy` with DNS).
`--icmp allow` is refused (only `pass` and `drop`), `--nat64` alone has no
DNS64, and `--loss` takes 0.01 to 20 percent where sand-net takes up to 49.

## Premise

The surface is `pod-netlab` of podcrates (v0.40.0 at the QA, v0.45.0 now,
`src/scenario.rs` and `src/knobs/check.rs`). `v0.45.0` takes `--icmp
allow` and gives the reason of each other difference in the README of
the crate; the five value sets stay open (podcrates@T-586, partial). The lab help says the guest
parts wait for podcrates (T-1662, issue 260).

## Approach

podvm asks podcrates for the missing scenarios and the sand-net values,
or for a documented reason per difference (proposed COORD issue), and
accept `allow` as a word for `pass`. podvm names the version of
`pod-netlab` and the absent scenarios in `docs/limits.md` and in the help
of `lab scenarios` meanwhile.

## Decision

The scope index (#193) makes sand-net the reference for the lab; a
difference is either closed or documented.

## Prove

`podvm lab plan --icmp allow` exits 0, and `podvm lab scenarios` lists each scenario of the sand-net table or `docs/limits.md` names each absent one

## Done

**Done 2026-11-15.** `podvm lab plan --icmp allow` exits 0: podcrates
`v0.45.0` takes `allow` as a word for `pass`, and podvm has `v0.50.0`.
`docs/limits.md`, section "Network lab", names each of the 12 absent
scenarios of sand-net and the reason (each needs a knob of issue 180 and
a live uplink or server, podcrates@T-348), and each kept difference with
its reason from the README of `pod-netlab` (`--loss` to 20 %, DNS64 as a
flag of its own, the nine knob sets with no source of sand-net,
podcrates@T-586). No new COORD issue is needed: the README of the crate
answers the ask of the Approach.


# T-1916: Lab profiles: each hostile host class as a profile that a project brings up

**Source:** PodUtils/podrunner#180 (COORD-061 of podrunner), COORD-140 (PodUtils/podvm#424)
**Category:** net
**Milestone:** none
**Priority:** P2
**Effort:** L
**Status:** partial

## Problem

The first user runs of podrunner found about 30 defects that CI did not find, because each runner of GitHub allows each operation; the defects come from host classes (no user namespace, a refused bind, inherited descriptors, a small `/tmp`, and others).

## Premise

The init of a guest of the machine tier is podvm, so a guest can set the sysctls, the mounts, a seccomp filter and the user of a profile. podvm's QA fixtures (`experiments/deny-chroot.c`, the fixture `deny-unshare`) are seccomp filters before an exec, and experiments 356 and 443 run on them.

## Approach

`run --podvm-tier=machine --podvm-lab PROFILE` for the full form; `podvm lab run PROFILE -- CMD` for the parts that a host with no root holds; the profiles as data from `pod-lab` of podcrates, else a table of podvm with the same names; `check` from the rows of `pod-hostprobe`.

## Decision

A profile that a host cannot hold is refused by name, part by part; a lab never allows what it names as refused.

## Prove

`podvm lab run userns-off -- unshare -Ur true` exits 1 with EPERM, `podvm lab check userns-off` exits 0, and a guest with `--podvm-lab userns-off` gives `0` for `cat /proc/sys/user/max_user_namespaces`.

## Done

**Blocked (2026-11-15).** The shape of the profiles: COORD-140 (PodUtils/podvm#424) asks whether podcrates takes `pod-lab` (PodUtils/podrunner#180, ask 1).

**Blocked, amended (2026-11-15, later).** podcrates takes `pod-lab` (COORD-107, PodUtils/podcrates#180, podcrates@T-692): part 1, in the tag after `v0.53.0`, gives the profiles as data, `check`, `run` with no root, and the binary `podlab`; part 2 gives the step of Actions and the machine-tier form that podvm drives (`--podvm-lab PROFILE`, the profiles from `pod-lab`). The table is the profiles of PodUtils/podrunner#180 with the six of PodUtils/podvm#424. Clears when a tag of podcrates carries part 1.

**Partial (2026-11-15).** The host form, as PodUtils/podrunner#182 asks before the crate (`crates/podvm/src/lab/host.rs`, `host/probe.rs`): `podvm lab profiles`, `podvm lab run PROFILE[,PROFILE...] -- CMD` and `podvm lab check PROFILE[,...]` for `bind-denied` (a seccomp filter: `bind` and `connect` with the length of a `sockaddr_in` or `sockaddr_in6` give EACCES; a filter cannot read the family, so an `AF_UNIX` path of exactly 13 or 25 bytes is refused too), `getsockname-denied` (EOPNOTSUPP), `userns-off` (`unshare` and `clone` with `CLONE_NEWUSER` give EPERM, `clone3` ENOSYS), `small-tmp` and `noexec-tmp` (a user and a mount namespace that keep the ids, a tmpfs of 1 GiB on `/tmp`, `TMPDIR=/tmp`), and `inherited-fds` (3 to 20 open with no `FD_CLOEXEC`). The rules compile with `pod_confine::deny::program`. `check` runs podvm's own probe in the lab through `/proc/self/exe`. Where the host refuses a user namespace, `small-tmp` refuses by name with exit 125 (measured under the `deny-unshare` fixture). Evidence (kernel 6.18, uid 1000): `experiments/462-lab-profiles.sh` PASS (`experiments/results/lab-profiles.txt`): each part of the six profiles holds, the control outside a lab does not hold, `unshare -Ur true` fails in `userns-off`, a copy of `true` in `/tmp` exits 126 in `noexec-tmp`. Remaining: the guest form (`--podvm-lab PROFILE` of the machine tier) and the profiles as data of `pod-lab`, after the tag of podcrates that carries part 1 (the blocker above).

**Partial (2026-11-15, pod-lab).** podvm takes podcrates `v0.54.0` (COORD-113, PodUtils/podcrates#188) and its `pod-lab` (podcrates@T-692 part 1): `podvm lab profiles`, `run` and `check` now take the 16 profiles of its table, its check against the rows of `pod-hostprobe`, and its run, which sets each part, checks the profile from inside the lab, and only then runs CMD (`crates/podvm/src/lab/host.rs`; podvm's own seccomp rules and probe left). podvm gives it its probe children and a lab flag of its own (`--podvm-lab-process`, from `userns::enter_if`, first in `main`), and keeps the list of profiles: a list runs as nested labs, with each profile of a tmpfs outermost (an outer `userns-off` refused the user namespace of an inner `small-tmp` with EPERM). Changed for a caller: `check` measures this host (0 holds, 1 fails, 3 unseen); `inherited-fds` holds descriptors 3 to 9. `experiments/462-lab-profiles.sh` PASS as uid 1000 (`experiments/results/lab-profiles.txt`): the six profiles of PodUtils/podrunner#182 as one list run `true`; `check` of the list on the host fails with 3 parts; `unshare -Ur` fails under `userns-off`; a copy of `true` in `/tmp` exits 126 under `noexec-tmp`; descriptors 3 to 9 are open under `inherited-fds`. Remaining: the guest form (`--podvm-lab PROFILE` of the machine tier), part 2 of podcrates@T-692.

# T-1924: `podvm lab run --way-out` for a live lane in a network class

**Source:** podcrates `v0.61.0` (PodUtils/podcrates#209, podcrates@T-746)
**Category:** net
**Milestone:** none
**Priority:** P2
**Effort:** S
**Status:** done

## Problem

podcrates `v0.61.0` gives `pod_lab::Options::way_out`: a `CONNECT` that the rules of a network class pass, to a name that is not of the lab, goes out to the real network through the stand-in proxy, and the command never gets the proxy of the host or its credential. `podvm lab run` passes `way_out: false` and has no flag for it, so a live lane of podvm in a network class cannot reach the network. podcrates asks each project for a live lane in a network class with the way out.

## Premise

Only a profile with a stand-in proxy takes the way out; `pod_lab::run` refuses it for another profile.

## Approach

A flag `--way-out` of `podvm lab run`, passed as `Options::way_out`, with a row of the parity table and a line of the help; then a live lane of a network class with it (for example `podvm pull` of a small image through the lab).

## Decision

Not in the take of `v0.61.0`: a new flag needs its help, its parity row and its lane.

## Prove

`podvm lab run --way-out proxy-only -- podvm pull IMAGE` (a profile with a stand-in proxy) exits 0, and the same with no `--way-out` fails at the `CONNECT`.

## Correction

2026-11-16. With no `--way-out`, the stand-in proxy answers a `CONNECT` to a name outside the lab with the server of the lab, so the pull fails at the first answer (the manifest or the TLS check), not at the `CONNECT`. The Prove reads "fails" for that case.

## Done

**Done, amended (2026-11-17).** podcrates `v0.64.0` answers COORD-162: the certificate file of a lab with a way out carries the roots of the host (podcrates@T-792), and the stand-in ignores SIGPIPE (podcrates@T-796). podvm took the tag and removed its own SIGPIPE handler. Measured as uid 1000 on the cloud host: `lab run --way-out proxy-only -- podvm pull mirror.gcr.io/library/alpine:3.20` exits 0 with no `SSL_CERT_FILE` in CMD, and the same with no `--way-out` exits 125. A host `SSL_CERT_FILE` that the user cannot read gives the lab no roots of the host (named in COORD-166, #459).

**Done (2026-11-16).** `podvm lab run --way-out` (`crates/podvm/src/lab/host.rs`, `run_flags`) sets `Options::way_out` for the lab of the stand-in proxy, and passes `--way-out` to an inner lab of a list that has one; a list with no such profile is a flag error (2). The usage and the parity table have the flag (and `--allow-connect`, which had no row). The lab process makes the process not dumpable before the way out starts, as `podlab` does. Measured as uid 1000 on the cloud host (`proxy-only`): `lab run --way-out proxy-only -- sh -c 'SSL_CERT_FILE=HOST-CA podvm pull mirror.gcr.io/library/alpine:3.20'` exits 0 ("Downloaded newer image"); the same with no `--way-out` exits 125 (the server of the lab answers). Two findings: (1) the stand-in died of SIGPIPE in each run of `proxy-only` on this host (a probe closed its socket before the answer; podvm sets SIGPIPE to its default, T-1769), so each later probe got "Connection refused" and the lab did not hold, also with no `--way-out`. The lab process now has a SIGPIPE handler that does nothing, which an `exec` of CMD sets back to the default. (2) The certificate file of the lab (`SSL_CERT_FILE`) holds only the CAs of the lab, and podvm, curl and most clients take that file in place of their roots, so a TLS client in a class that does not terminate TLS cannot check a real server through the way out. The Prove sets the CA of the host in CMD; COORD-162 asks podcrates to add the roots of the host to the file where the lab has a way out. Unit tests: `lab::host::tests` (6).

# T-1961: The donor study: the network gateway

**Source:** PodUtils/podvm#578, the donor study of 2026-11-17
**Category:** net
**Milestone:** none
**Priority:** P3
**Effort:** M
**Status:** open

## Problem

The gateway has no ICMP echo relay (T-1664 is blocked), no way to publish a host port after start, no mark on its own sockets and no RST on a failed dial. The code lives in `pod-netstack`, `pod-proxy` and `pod-warp`.

## Premise

No item serves podrunner directly. Ask podcrates by COORD issue first.

## Approach

1. TH-50: relay ICMP echo through ping sockets.
2. TH-51: add runtime expose and unexpose.
3. TH-52: mark the gateway's sockets; answer RST.
4. TH-53, TH-54, TH-55 as the issue lists them.

## Decision

-

## Prove

In a lab guest with no `CAP_NET_RAW`, `ping` to a host address gets an echo reply through the relay.
