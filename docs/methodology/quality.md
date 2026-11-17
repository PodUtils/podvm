# Quality

This page gives the plan that follows the external QA of
`0.2.0-beta.31` (the owner's ruling "Quality first", 2026-11-17). Each
item has an entry in `TODO/`.

## What went wrong

On 2026-11-17 podvm said that `0.2.0-beta.31` was ready for testers. In
about one hour an external tester confirmed 37 faults with the documents
and the binary only (issues 472 to 508), and more came after. The gate,
the integration tests and the experiments had passed. They had proved
that payloads start. They had not proved what a user does next.

The faults fall into five classes:

| Class | Reports | Cause |
| --- | --- | --- |
| The surface of the CLI | 478, 479, 482 to 488, 492 to 494, 497, 500, 519, 520 | Three texts that people write by hand (the parser, the help, the parity table) disagree, and nothing compares them. |
| A hostile input | 505, 506 | A panic (exit 134) on an argument that is not UTF-8, or on a number that overflows. |
| Shared state | 472, 475, 501 | Two processes write the store at once. The invariant (I5) lived in comments, and one writer of a crate did not keep it. |
| A message that is not true | 491, 499, 507, 508, 510 | A banner or a field says what podvm asked for, not what the entered mechanism did. |
| A gap of a mechanism | 473, 474, 476, 477, 480, 489, 490, 495, 496, 498, 502, 503 | Each one has its own cause. |

The first four classes are failures of method, not bad luck. Each one
can be found again by one check, or prevented by one structure.

## The rules

1. **No "ready" from podvm's own checks.** A release note says what the
   checks prove and lists the known gaps. Only an outside pass (rule 5)
   gives evidence of readiness.
2. **Fail safe at each edge, with a fallback.** Code that can fail gets a
   second path, whether or not that path is ever used:
   - a panic becomes one named line and exit 125 (a panic hook);
   - a number goes through one checked parser with a bound, and a value
     out of the bound is refused, never clamped or wrapped;
   - a write of the store holds a lock *and* each sweep waits for an age;
   - a lost write of the end of a run goes to a side file that the next
     reconcile reads.
3. **One source of truth for the surface.** A flag exists where its
   parity row is. One table-driven check per verb compares the parity
   rows, `--help` and the parser.
4. **Say what was done, not what was asked.** Each banner and each
   inspect field is written from the mechanism after it entered.
5. **An outside pass before each beta.** An agent with only the binary
   and `podvm man`, and no source, works for one hour. Its P0 and P1
   findings block the beta.

## What podvm does not do

podvm does not add one test for each report. A fault gets a regression
test only where the test is cheap and pins a class (rules 2 and 3). The
check of rule 3 is one file of tests, not one test for each flag.

## The entries

| Entry | Item |
| --- | --- |
| T-1946 | The contract of the CLI: one table-driven harness (rule 3, and the hostile inputs of rule 2). |
| T-1947 | Fail safe at the edges: the panic hook, the checked numbers, the writes of the store (rule 2). |
| T-1948 | Messages from the entered mechanism (rule 4). |
| T-1949 | The gate of a beta: `workloads.yml`, the harness, the outside pass, and the release note of known gaps (rules 1 and 5). |

The faults of a crate of podcrates go to podcrates as COORD issues; podvm
takes the tag.
