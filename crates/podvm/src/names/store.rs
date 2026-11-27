//! PodUtils/podrunner#168 (TODO/cli.md T-1913): `--store DIR`, the store
//! of each verb, with the meaning of `PODVM_STORE`. It stands before the
//! verb (`podvm --store DIR images`) or right after it (`podvm images
//! --store DIR`); a word further on belongs to the verb or the payload,
//! and stays.

/// Take `--store DIR` (or `--store=DIR`) out of `argv` and set
/// `PODVM_STORE` to DIR. A `--store` with no value is a flag error.
///
/// ⛔ Call it first in `main`, with one thread (it sets a variable of the
/// process).
pub fn take(mut argv: Vec<String>) -> Vec<String> {
    for at in [1usize, 2] {
        let Some(word) = argv.get(at).cloned() else {
            break;
        };
        let value = if word == "--store" {
            if argv.len() <= at + 1 || argv[at + 1].is_empty() {
                eprintln!("podvm: --store takes a directory (TODO/cli.md T-1913)");
                std::process::exit(podvm_image::error::EXIT_FLAG_ERROR);
            }
            let v = argv.remove(at + 1);
            argv.remove(at);
            v
        } else if let Some(v) = word.strip_prefix("--store=") {
            let v = v.to_string();
            argv.remove(at);
            v
        } else {
            continue;
        };
        if super::bytes::word("", &value, super::bytes::Class::Podvm).is_err() {
            std::process::exit(podvm_image::error::EXIT_FLAG_ERROR); // T-2087
        }
        // SAFETY: one thread (the contract of this function).
        unsafe { std::env::set_var("PODVM_STORE", &value) };
        break;
    }
    argv
}

#[cfg(test)]
mod tests {
    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_store_flag_stands_before_or_right_after_the_verb() {
        assert_eq!(
            super::take(v(&["podvm", "--store", "/s", "images"])),
            v(&["podvm", "images"])
        );
        assert_eq!(
            super::take(v(&["podvm", "images", "--store=/s"])),
            v(&["podvm", "images"])
        );
        // A word of the payload stays.
        let run = v(&["podvm", "run", "img", "sh", "--store", "/x"]);
        assert_eq!(super::take(run.clone()), run);
    }
}
