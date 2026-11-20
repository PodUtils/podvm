//! TODO/enter.md T-1984: the exec rule of the program that an exec of the
//! dynamic loader runs ([`super::ldso`]), on the arguments of one exec.
//!
//! ⛔ No allocation: each buffer is in the caller's [`Scratch`].

use super::*;

/// Whether a static program enters through podvm's handler
/// ([`dispatch_into`]): podvm named itself and the table has a root.
pub(super) fn dispatches(t: &Table) -> bool {
    let named =
        unsafe { crate::map::lookup_env(DISPATCH_SELF_VAR) }.is_some_and(|(_, n)| n > 0 && n < OUT);
    named
        && t.pairs[..t.n].iter().any(|pr| {
            pr.from_n == 1 && unsafe { *pr.from_p } == b'/' && pr.to_n > 0 && pr.to_n < OUT
        })
}

/// TODO/enter.md T-1984: whether the exec table lets each argument run
/// that the loader of this exec can take as its program
/// ([`ldso::each_program`]). The arguments are the kernel's: those of each
/// `#!` level from the deepest, then `file0`, then `argv` after its first.
/// Through podvm's handler (`dispatch`) the loader opens a guest path,
/// which the table resolves; else the kernel runs the loader, which opens
/// the name on the host. A name that does not open runs nothing and
/// passes; a name that cannot resolve fails safe.
///
/// # Safety
/// `file0`, `argv` and each level as the exec's own contract. Writes
/// `s.guest`, `s.exe` and `s.lib`, which hold nothing at this step.
pub(super) unsafe fn loader_program_allowed(
    t: &Table,
    levels: &[(*const c_char, *const c_char)],
    file0: *const c_char,
    argv: *const *const c_char,
    dispatch: bool,
    s: &mut Scratch,
) -> bool {
    let mut lead = [core::ptr::null::<c_char>(); 2 * DEPTH + 1];
    let mut k = 0usize;
    if let Some(last) = levels.last() {
        let mut push = |p: *const c_char| {
            if !p.is_null() {
                lead[k] = p;
                k += 1;
            }
        };
        push(last.1);
        for l in levels.iter().rev().skip(1) {
            push(l.0);
            push(l.1);
        }
        push(file0);
    }
    let bytes = |p: *const c_char| unsafe {
        core::slice::from_raw_parts(p as *const u8, crate::map::strnlen(p, OUT))
    };
    let rest = (1..)
        .map(|i| {
            if argv.is_null() {
                core::ptr::null()
            } else {
                unsafe { *argv.add(i) }
            }
        })
        .take_while(|p| !p.is_null());
    let args = lead[..k].iter().copied().chain(rest).map(bytes);
    let (guest, rguest, host) = (&mut s.guest, &mut s.exe, &mut s.lib);
    ldso::each_program(args, |p| {
        if p.len() >= OUT - 1 {
            return false;
        }
        let name = p.as_ptr() as *const c_char;
        if dispatch {
            let n = unsafe { crate::map::absolutize(t, AT_FDCWD, name, guest) };
            let Ok(n) = usize::try_from(n) else {
                return false;
            };
            if n == 0 {
                return false;
            }
            let Ok((_, hn)) = (unsafe { walk::resolve(t, &guest[..n], rguest, host) }) else {
                return false;
            };
            let Some(access) = crate::next_access() else {
                return false;
            };
            let absent = unsafe { access(host.as_ptr() as *const c_char, 0) } != 0;
            absent || unsafe { crate::map::exec_allowed(&host[..hn]) }
        } else {
            match unsafe { crate::map::link_of_path(AT_FDCWD, name, true, host) } {
                crate::map::Link::At(hn) => unsafe { crate::map::exec_allowed(&host[..hn]) },
                crate::map::Link::Absent => true,
                crate::map::Link::Unknown => false,
            }
        }
    })
}
