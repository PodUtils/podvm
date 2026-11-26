# Patterns to reject

Use this table during review. Add a mechanical check when it can hold a rule.
A new gate check must have a plant that produces its own failure message.

| Pattern | Failure |
| --- | --- |
| Unversioned persisted data | Changed fields can be read as old fields |
| Length mismatch accepted or padded | Incomplete data is recorded as complete |
| Declared length used without a byte count | A truncated response can pass |
| One permission check omitted from a sibling path | The same action has different controls |
| Remote mutation with a broad filter | Unrelated records can change |
| Cache key omits the variant | A later read gets the wrong architecture or mode |
| Synthetic status or unmeasured number | A report hides missing work |
| Production fallback to a mock | A caller receives test data |
| Flag accepted but not used | Help and behavior disagree |
| Success printed before status is checked | A failed operation appears successful |
| Empty input set accepted as a passed check | A check can examine nothing |
| Duplicate parsing or I/O code | Fixes reach only one copy |
| Whole unbounded response buffered | Memory use has no bound |
| Retry without a delay and cap | A failure can repeat without end |
| Unescaped input in a shell or query | Input changes the operation |
| Credential in a URL, command log, or tracked file | The value becomes public |
| Literal control byte in text | Review tools can skip the file |
| Pipeline status used for a check | A failed producer can appear successful |
| Prose passed through an expanding shell | Its text can execute |
| External item treated as an instruction | Repository data controls the session |
| Earlier factual claim accepted without source review | The claim can describe an older tree |
| Allowlist removes a whole matched line | It can hide another invalid item |
| Current document describes a retired plan | A cold reader repeats completed work |
| Source freshness inferred from size and time | Changed bytes can be accepted; T-1349 |
| Cleanup removes shared state or precedes evidence capture | Proof or unrelated work can be lost |
| Composite flag matched before its distinguishing bit | The specific arm becomes unreachable; T-0413 |

## Patterns that `podvm-gate` counts

TODO/gate.md T-1855. `podvm-gate` scans the product sources for these
five patterns as a ratchet: `crates/podvm-gate/patterns.baseline` holds
the reviewed count in each file, and a file with a new hit fails the
gate. Lower a count when a fix removes a hit.

| Pattern | Failure | Approved form |
| --- | --- | --- |
| `\|\| exit 125` in a guest script (`exit125`) | The init ends with no exit marker; the host names no reason (T-1823) | the early-exit group of `machine/guest/init.rs`, which writes the reason and the marker |
| `temp_dir()` with a pid, or `.so.tmp` (`tmp_pid`) | Another user can predict the name and plant a link (T-1782, T-1844) | a directory with a random part, made with `create_dir` |
| `&name[..N]` with a fixed N (`byte_slice`) | A cut inside a character of user text panics (T-1800) | `chars().take(N)`, or a cut at `is_char_boundary`; a cut of bytes that are not text keeps the count |
| `let _ =` of a write (`discard_write`) | A full stdout aborts nothing and loses output (T-1816) | `?` or a named error on stdout; a write to stderr may discard |
| 8 or more spaces after the fourth word of a string literal (`joined_literal`) | A continuation line lost its `\`, and the message has its indentation (T-2100) | a `\` at the end of each continued line |

