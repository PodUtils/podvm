//! The ownership memo: what the payload MEANT a file's owner to be.
//!
//! [`TODO/interpose.md`](../../../TODO/interpose.md) T-0704 and T-0710.
//! `chown 0:42` answers `EINVAL` on this runtime, so podvm records the intent
//! and reports it back from the `stat` family. The model is fakeroot's
//! `references/salsa-debian__fakeroot/tree/libfakeroot.c:875-907`: read the real
//! metadata, overwrite `st_uid` and `st_gid` with the intended values, memo
//! them, attempt the real call, and swallow the failure.
//!
//! ⛔ **A FILE ON THE HOST, BESIDE THE CONTAINER RECORD, NOT INSIDE THE ROOTFS.**
//! T-0710's ruling. The payload cannot read it, forge it, truncate it or
//! delete it. podvm opens it before the `chroot` and hands the interposer a
//! descriptor to it at spawn (`PODVM_MEMO_FD`), re-handed on every `exec`
//! re-entry. An entry that would load the tier without one refuses instead of
//! starting with no memo. Nothing on the host reads this file to decide
//! anything: it is the payload's own view of ownership, not evidence.
//!
//! ⛔ **IT MUST CROSS PROCESSES, and that is what rules out a table in memory.**
//! T-0704's own `Prove` is `sh -c 'chown 0:42 /tmp/f && stat -c %u:%g /tmp/f'`:
//! `chown` and `stat` are two `execve`s, so a memo that lived in this object's
//! own memory would be gone before the question was asked. fakeroot solves this
//! with a DAEMON, `faked`, which podvm has no room for inside somebody else's
//! chroot. `O_APPEND` and one 32-byte write per record is what replaces the
//! daemon and the lock both: the host opens with `O_APPEND`, so the kernel
//! serialises concurrent writers and this object holds no lock (T-0701
//! constraint 2 by not needing one).
//!
//! ⚠ **What it is not.** It changes no kernel permission check, exactly as
//! [`TODO/extract.md`](../../../TODO/extract.md) T-0302's sidecar does not: a
//! file recorded here as `gid 42` is owned by the running id on disk, and every
//! access check the kernel makes uses the latter.

use core::ffi::{c_int, c_void};

use crate::say;

/// The variable carrying the memo descriptor number into the payload.
///
/// ⚠ Read straight out of the process's own environ like `map` does: no libc
/// call, no lock, no allocation. The host sets it alongside `LD_PRELOAD` in
/// the same `apply` call, so the two cannot disagree about whether the tier
/// is loaded.
pub const MEMO_FD_VAR: &[u8] = b"PODVM_MEMO_FD";

/// The variable naming the memo's file: `DEV:INO` of the host file, in
/// decimal (TODO/interpose.md T-1928, PodUtils/podrunner#197).
///
/// ⛔ A payload can close the memo's descriptor and get the number again
/// for a file of its own: npm 6 closes each inherited descriptor above 2,
/// and Node's next `open` got 17. A lookup then moved that file's offset to
/// its end, and a record wrote 32 bytes into it. So each use checks that
/// the descriptor is still this file, and a descriptor that is not is no
/// memo at all.
pub const MEMO_ID_VAR: &[u8] = b"PODVM_MEMO_ID";

/// The variable naming the memo's host path (TODO/interpose.md T-2000).
///
/// ⛔ apt marks each descriptor above 2 close-on-exec before it runs dpkg
/// (`ExecFork`), and npm 6 closes them: the `tar` under `dpkg-deb` then had
/// no memo, and `apt-get install` on the userland rung failed with "Cannot
/// change ownership" (the fifth outside pass, OP-04). Where the descriptor
/// is gone, the process opens this path itself, by the raw call so that the
/// path table does not map it, and uses the file only where it is the file
/// of [`MEMO_ID_VAR`].
pub const MEMO_PATH_VAR: &[u8] = b"PODVM_MEMO_PATH";

/// The memo that this process opened from [`MEMO_PATH_VAR`]; -1 where it
/// has not tried, and [`GONE`] where the path did not open (a chroot, where
/// the host path is not there): each process tries once.
static REOPENED: core::sync::atomic::AtomicI32 = core::sync::atomic::AtomicI32::new(-1);

/// One record. ⛔ Fixed width and little-endian, so a reader can scan without
/// parsing and without allocating.
///
/// ```text
///   0  dev   u64      the kernel's device number for the file
///   8  ino   u64      and its inode. ⭐ NOT the path: a path is renamed,
///                     hard-linked and resolved through symlinks, and the
///                     question "who owns this file" is about the inode
///  16  uid   u32
///  20  gid   u32
///  24  set   u32      bit 0 the uid is meant, bit 1 the gid
///  28  pad   u32      so the record is 32 bytes and a scan is one shift
/// ```
pub const RECORD: usize = 32;
pub const SET_UID: u32 = 1;
pub const SET_GID: u32 = 2;

/// ⚠ How much of the memo a lookup will read. Bounded, because this runs on
/// every `stat` a payload makes: 4 MiB is 131,072 records, and a container that
/// has recorded more than that has a different problem. ⛔ Reaching it is a
/// refusal, never a stale answer (T-0710 check G).
const SCAN_CEILING: usize = 4 * 1024 * 1024;

unsafe extern "C" {
    fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
    fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
    fn lseek(fd: c_int, offset: i64, whence: c_int) -> i64;
}

const SEEK_SET: c_int = 0;
const SEEK_END: c_int = 2;

/// What the payload meant, for one file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Owner {
    pub uid: u32,
    pub gid: u32,
    pub set: u32,
}

/// What a lookup established.
///
/// ⛔ Three outcomes, not two. `Miss` means no record names this file and the
/// real `stat` answers. `BeyondCeiling` means the memo holds more than a
/// lookup reads, so any answer from the scanned prefix could be stale: the
/// caller falls back to the real `stat`, marked degraded, never a record from
/// before the ceiling. Collapsing the last two is how a `chown` that succeeded
/// reads back as the original uid.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lookup {
    Hit(Owner),
    Miss,
    BeyondCeiling,
}

fn u64_le(b: &[u8]) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[..8]);
    u64::from_le_bytes(a)
}

fn u32_le(b: &[u8]) -> u32 {
    let mut a = [0u8; 4];
    a.copy_from_slice(&b[..4]);
    u32::from_le_bytes(a)
}

/// The memo descriptor handed at spawn, or `None`.
///
/// Decimal digits only; anything else is no descriptor rather than a guess at
/// one. A negative number, an empty value and a non-numeric one are all the
/// same absence: the host always hands a valid one where the tier loads, so
/// anything else is a process that was not started through it.
///
/// `pub(crate)` for the emulation tally, which rides the same descriptor.
///
/// ⛔ T-1928: only where the descriptor is still the file of
/// [`MEMO_ID_VAR`]. A missing or bad identity is no memo, never a guess.
pub(crate) fn memo_fd() -> Option<c_int> {
    let id = handed_id()?;
    // The check must not change what the payload sees: a failed `fstat`
    // sets `errno`, and this runs inside calls that succeeded.
    let saved = crate::errno();
    let r = memo_fd_of(id);
    crate::set_errno(saved);
    r
}

const GONE: c_int = -2;

/// Forget the descriptor of [`reopen`], for a test that changes the
/// variables.
#[cfg(test)]
pub(crate) fn forget_reopened() {
    REOPENED.store(-1, core::sync::atomic::Ordering::Release);
}

fn memo_fd_of(id: (u64, u64)) -> Option<c_int> {
    use core::sync::atomic::Ordering;
    if let Some(fd) = handed_fd()
        && unsafe { file_id(fd) } == Some(id)
    {
        return Some(fd);
    }
    let have = REOPENED.load(Ordering::Acquire);
    if have == GONE {
        return None;
    }
    if have >= 0 && unsafe { file_id(have) } == Some(id) {
        return Some(have);
    }
    let Some(fd) = reopen(id) else {
        REOPENED.store(GONE, Ordering::Release);
        return None;
    };
    match REOPENED.compare_exchange(have, fd, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => Some(fd),
        Err(other) => {
            // Another thread opened it first: keep one descriptor.
            unsafe {
                crate::raw_calls::forward(crate::raw_calls::SYS_CLOSE, [fd as _, 0, 0, 0, 0, 0])
            };
            (other >= 0 && unsafe { file_id(other) } == Some(id)).then_some(other)
        }
    }
}

/// Open the file of [`MEMO_PATH_VAR`] where it is the file `id`.
fn reopen(id: (u64, u64)) -> Option<c_int> {
    const O_RDWR: i64 = 2;
    const O_APPEND: i64 = 0o2000;
    const O_CLOEXEC: i64 = 0o2000000;
    const AT_FDCWD: i64 = -100;
    let (p, n) = unsafe { crate::map::lookup_env(MEMO_PATH_VAR) }?;
    let mut path = [0u8; 4096];
    if n == 0 || n >= path.len() {
        return None;
    }
    let bytes = unsafe { core::slice::from_raw_parts(p, n) };
    if bytes[0] != b'/' || bytes.contains(&0) {
        return None;
    }
    path[..n].copy_from_slice(bytes);
    let args = [
        AT_FDCWD as _,
        path.as_ptr() as _,
        (O_RDWR | O_APPEND | O_CLOEXEC) as _,
        0,
        0,
        0,
    ];
    // SAFETY: openat(2) of a NUL-ended path that lives for the call.
    let fd = unsafe { crate::raw_calls::forward(crate::raw_calls::SYS_OPENAT, args) };
    if fd < 0 || fd > c_int::MAX as _ {
        return None;
    }
    let fd = fd as c_int;
    if unsafe { file_id(fd) } == Some(id) {
        Some(fd)
    } else {
        unsafe { crate::raw_calls::forward(crate::raw_calls::SYS_CLOSE, [fd as _, 0, 0, 0, 0, 0]) };
        None
    }
}

/// The device and inode of the file on `fd`, through the real `fstat`.
///
/// # Safety
/// A plain system call on a descriptor number; any number is safe to ask.
unsafe fn file_id(fd: c_int) -> Option<(u64, u64)> {
    let fstat = crate::next_fstat()?;
    let mut st = [0u8; crate::procfs::STAT_LEN];
    if unsafe { fstat(fd, st.as_mut_ptr() as *mut c_void) } != 0 {
        return None;
    }
    Some(unsafe { crate::dev_ino(st.as_ptr() as *const c_void) })
}

/// `DEV:INO` of [`MEMO_ID_VAR`], decimal, or `None`.
fn handed_id() -> Option<(u64, u64)> {
    let (p, n) = unsafe { crate::map::lookup_env(MEMO_ID_VAR) }?;
    if n == 0 || n > 41 {
        return None;
    }
    let bytes = unsafe { core::slice::from_raw_parts(p, n) };
    let colon = bytes.iter().position(|&c| c == b':')?;
    Some((decimal(&bytes[..colon])?, decimal(&bytes[colon + 1..])?))
}

fn decimal(b: &[u8]) -> Option<u64> {
    if b.is_empty() || b.len() > 20 {
        return None;
    }
    let mut v: u64 = 0;
    for c in b {
        if !c.is_ascii_digit() {
            return None;
        }
        v = v.checked_mul(10)?.checked_add(u64::from(c - b'0'))?;
    }
    Some(v)
}

/// The number of [`MEMO_FD_VAR`], before the check of [`memo_fd`].
fn handed_fd() -> Option<c_int> {
    let (p, n) = unsafe { crate::map::lookup_env(MEMO_FD_VAR) }?;
    if n == 0 || n > 10 {
        return None;
    }
    let bytes = unsafe { core::slice::from_raw_parts(p, n) };
    let mut v: i64 = 0;
    for c in bytes {
        if !c.is_ascii_digit() {
            return None;
        }
        v = v.checked_mul(10)?.checked_add((c - b'0') as i64)?;
    }
    if v < 0 || v > c_int::MAX as i64 {
        return None;
    }
    Some(v as c_int)
}

/// Append what the payload meant, through the handed descriptor.
///
/// ⛔ **One `write` per record at the end of the file.** The host opens
/// `O_APPEND`, so the kernel makes the seek and the write of an appending
/// descriptor one operation and concurrent writers cannot interleave a record:
/// no lock here at all. The explicit seek makes the position exact where the
/// descriptor was opened read-write without append (a test harness holding
/// the same file for reading); under `O_APPEND` the kernel still appends, so
/// the seek changes nothing there.
///
/// ⚠ A record is APPENDED rather than replacing an earlier one for the same
/// inode. A rewrite would need a lock and a scan on the write path; the reader
/// takes the LAST matching record, so the newest intent wins.
///
/// Where no descriptor was handed this returns false and the caller hands back
/// the real failure rather than hiding it: a success reported with no memo
/// behind it is the lie this object exists to avoid.
pub fn record(dev: u64, ino: u64, o: Owner) -> bool {
    let Some(fd) = memo_fd() else {
        return false;
    };
    let mut rec = [0u8; RECORD];
    rec[0..8].copy_from_slice(&dev.to_le_bytes());
    rec[8..16].copy_from_slice(&ino.to_le_bytes());
    rec[16..20].copy_from_slice(&o.uid.to_le_bytes());
    rec[20..24].copy_from_slice(&o.gid.to_le_bytes());
    rec[24..28].copy_from_slice(&o.set.to_le_bytes());
    // ⚠ End first, so a read-write descriptor without `O_APPEND` still lands
    // past every record rather than overwriting the first one.
    if unsafe { lseek(fd, 0, SEEK_END) } < 0 {
        return false;
    }
    let n = unsafe { write(fd, rec.as_ptr() as *const c_void, RECORD) };
    n == RECORD as isize
}

/// What the payload last meant for this file.
///
/// ⚠ Scanned forwards through fixed-size chunks so nothing allocates and the
/// buffer is one page of stack. The LAST match in the scanned prefix wins.
///
/// ⛔ **Bounded, and the bound refuses.** Where the memo holds more than
/// `SCAN_CEILING` bytes the answer is `BeyondCeiling` whatever the prefix
/// held: a newer record past the ceiling could name this file, so a record
/// from before it could be stale. The caller then answers the real `stat`,
/// marked degraded. Returning the prefix's last match instead is a wrong
/// answer that exits 0 (T-0710).
pub fn lookup(dev: u64, ino: u64) -> Lookup {
    let Some(fd) = memo_fd() else {
        return Lookup::Miss;
    };
    // ⛔ Rewind first: `chown` leaves the offset at the end, and a `stat` in
    // the same process would otherwise read EOF and report no memo.
    if unsafe { lseek(fd, 0, SEEK_SET) } < 0 {
        return Lookup::Miss;
    }
    // ⛔ 4 KiB, a multiple of the record size, so a record never straddles two
    // reads and the scan needs no carry-over buffer.
    const CHUNK: usize = 128 * RECORD;
    let mut buf = [0u8; CHUNK];
    let mut found: Option<Owner> = None;
    let mut scanned = 0usize;
    loop {
        let mut have = 0usize;
        // ⚠ `read` may return short. Filled to a record boundary before the
        // scan, or a partial record at the end of a chunk would be read as a
        // whole one.
        while have < CHUNK {
            let n = unsafe { read(fd, buf[have..].as_mut_ptr() as *mut c_void, CHUNK - have) };
            if n <= 0 {
                break;
            }
            have += n as usize;
        }
        if have == 0 {
            break;
        }
        let mut at = 0usize;
        while at + RECORD <= have {
            let r = &buf[at..at + RECORD];
            if u64_le(&r[0..8]) == dev && u64_le(&r[8..16]) == ino {
                // ⚠ The LAST match in the scanned prefix wins, so this
                // overwrites rather than breaking out.
                found = Some(Owner {
                    uid: u32_le(&r[16..20]),
                    gid: u32_le(&r[20..24]),
                    set: u32_le(&r[24..28]),
                });
            }
            at += RECORD;
        }
        scanned += have;
        if have < CHUNK {
            break;
        }
        if scanned >= SCAN_CEILING {
            // ⛔ Announced, always, and not only under the debug switch: an
            // answer podvm stopped looking for is not an answer. The return
            // is the refusal; the prefix's candidate, if any, is dropped
            // rather than reported, because a newer record past the ceiling
            // could name the same file.
            say::line(&[
                b"the ownership memo holds more than ",
                say::Num::new(SCAN_CEILING as u64).as_bytes(),
                b" bytes and podvm stopped scanning it. The real owner is \
                  reported, marked degraded (TODO/interpose.md T-0710)",
            ]);
            return Lookup::BeyondCeiling;
        }
    }
    match found {
        Some(o) => Lookup::Hit(o),
        None => Lookup::Miss,
    }
}

/// Parse a decimal descriptor number, for the host-side tests.
///
/// The interposer itself reads the environ; this is the same grammar over a
/// byte string so `podvm` and the tests agree on what counts as handed.
pub fn parse_fd(spec: &[u8]) -> Option<c_int> {
    if spec.is_empty() || spec.len() > 10 {
        return None;
    }
    let mut v: i64 = 0;
    for c in spec {
        if !c.is_ascii_digit() {
            return None;
        }
        v = v.checked_mul(10)?.checked_add((c - b'0') as i64)?;
    }
    if v < 0 || v > c_int::MAX as i64 {
        return None;
    }
    Some(v as c_int)
}

#[cfg(test)]
mod tests;
