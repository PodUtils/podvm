#!/bin/sh
# Question: how often does the emulator end in the first seconds of a
# guest with four vCPUs under TCG, on a busy host?
#
# TODO/machine.md T-2016, found by the measures of T-2008: QEMU 8.2.2 ended
# on SIGSEGV in the boot of the guest of `run --podvm-init=image` (four
# vCPUs, call C26). The fault of QEMU is "system/physmem: fix
# use-after-free with dispatch" (qemu-devel, 2025-07; in QEMU 10.1 and the
# stable 10.0.4): a guest with more than one CPU changes the memory map
# during its PCI setup while another vCPU thread reads it.
#
# The script starts the emulator line of the machine tier N times, each for
# 7 s, while LOAD shells keep the CPUs of the host busy, and counts each
# start whose emulator ended by itself. A guest that lives 7 s is past its
# PCI setup. It starts no command of an image: `rdinit=/bin/sh` waits on
# the console.
#
# Pinned inputs:
#   PODVM_469_KERNEL  the guest kernel (a `vmlinuz-*` of `STORE/machine/`)
#   PODVM_469_INITRD  an initrd with `/bin/sh` (a `base-*.cpio` of
#                     `STORE/machine/`, which a run of the machine tier makes)
#   PODVM_469_QEMU    the emulator (default `qemu-system-x86_64` of PATH)
#   PODVM_469_N       the starts (default 30)
#   PODVM_469_LOAD    the busy shells (default the CPUs of the host)
#   PODVM_469_SMP     the vCPUs (default 4)
#   PODVM_469_SLOTS   the 9p devices (default 4, as a guest of a saved state)
#
# Exit 0 with the count; 2 where an input is missing. The transcript is
# appended to.

set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT="$ROOT/experiments/results/tcg-smp-start.txt"
QEMU="${PODVM_469_QEMU:-qemu-system-x86_64}"
KERNEL="${PODVM_469_KERNEL:-}"
INITRD="${PODVM_469_INITRD:-}"
N="${PODVM_469_N:-30}"
LOAD="${PODVM_469_LOAD:-$(nproc)}"
SMP="${PODVM_469_SMP:-4}"
SLOTS="${PODVM_469_SLOTS:-4}"
command -v "$QEMU" >/dev/null 2>&1 || { echo "SKIP: no $QEMU" >&2; exit 2; }
command -v python3 >/dev/null 2>&1 || { echo "SKIP: no python3" >&2; exit 2; }
{ [ -f "$KERNEL" ] && [ -f "$INITRD" ]; } || {
	echo "SKIP: give PODVM_469_KERNEL and PODVM_469_INITRD" >&2
	exit 2
}
[ -w /dev/kvm ] && echo "note: /dev/kvm opens, and this script measures TCG" >&2
WORK=$(mktemp -d /var/tmp/podvm-469.XXXXXX)
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/slot" "$(dirname "$OUT")"
say() { printf '%s\n' "$*" | tee -a "$OUT"; }

say "# $(date -u +%Y-%m-%dT%H:%M:%SZ) $("$QEMU" --version | head -1), host $(uname -r), $(nproc) CPUs, $SMP vCPUs, $SLOTS 9p devices, $LOAD busy shells, $N starts of 7 s"
python3 -I - "$QEMU" "$KERNEL" "$INITRD" "$WORK" "$N" "$LOAD" "$SMP" "$SLOTS" <<'EOF' | tee -a "$OUT"
import os, socket, subprocess, sys, time

qemu, kernel, initrd, work = sys.argv[1:5]
n, load, smp, slots = (int(x) for x in sys.argv[5:9])


def line(i):
    err = f"{work}/err-{i}.sock"
    a = [qemu, "-M", "pc", "-nodefaults", "-no-user-config", "-display", "none",
         "-serial", "stdio", "-no-reboot", "-smp", str(smp), "-nic", "user,model=virtio-net-pci",
         "-accel", "tcg,thread=multi,tb-size=512", "-cpu", "max", "-m", "3072M",
         "-kernel", kernel, "-initrd", initrd,
         "-append", "console=ttyS0 quiet loglevel=0 panic=-1 rdinit=/bin/sh",
         "-chardev", f"socket,id=podvmerr,path={err},server=on,wait=on", "-serial", "chardev:podvmerr"]
    for k in range(slots):
        a += ["-virtfs", f"local,path={work}/slot,mount_tag=podvm{k},security_model=none,readonly=on"]
    a += ["-qmp", f"unix:{work}/qmp-{i}.sock,server=on,wait=off"]
    return a, err


burn = [subprocess.Popen(["sh", "-c", "while :; do :; done"]) for _ in range(load)]
ended = 0
try:
    for i in range(n):
        argv, err = line(i)
        p = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
                             stderr=subprocess.DEVNULL)
        s = socket.socket(socket.AF_UNIX)
        for _ in range(500):
            try:
                s.connect(err)
                break
            except OSError:
                time.sleep(0.01)
        start = time.time()
        while time.time() - start < 7 and p.poll() is None:
            time.sleep(0.05)
        code = p.poll()
        if code is None:
            p.kill()
            p.wait()
        else:
            ended += 1
            how = f"signal {-code}" if code < 0 else f"exit code {code}"
            print(f"start {i}: the emulator ended with {how} after {time.time() - start:.1f} s",
                  flush=True)
        s.close()
        for f in (err, f"{work}/qmp-{i}.sock"):
            try:
                os.unlink(f)
            except FileNotFoundError:
                pass
finally:
    for b in burn:
        b.kill()
print(f"{ended} of {n} starts ended early")
EOF
left=$(pgrep -f "$WORK" || true)
[ -z "$left" ] || { say "A PROCESS STAYS: $left"; exit 1; }
exit 0
