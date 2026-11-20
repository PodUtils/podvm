# Licence texts kept for a package

A package of crates.io that ships no licence text of its own gets the texts
of its upstream repository here, in a directory `NAME-VERSION`, so that
`podvm-release-licenses` can retain a text for it. The tool reads this
directory only where the package and its workspace root hold no text.

| Package | Licence | Source of the texts |
| --- | --- | --- |
| `asn1-rs-impl` 0.2.0 | MIT OR Apache-2.0 | rusticata/asn1-rs at tag `asn1-rs-0.7.2` (`e9ea89e`), whose `impl/` is `asn1-rs-impl` 0.2.0: `LICENSE-MIT` and `LICENSE-APACHE` of the root |
