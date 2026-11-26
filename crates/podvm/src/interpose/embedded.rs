//! Decode and validate an embedded interposer on its first use.

use std::io::Read;
use std::sync::OnceLock;

use flate2::bufread::GzDecoder;

pub(super) struct Embedded {
    gzip: &'static [u8],
    raw_size: &'static str,
    decoded: OnceLock<Option<Box<[u8]>>>,
}

impl Embedded {
    const fn new(gzip: &'static [u8], raw_size: &'static str) -> Self {
        Self {
            gzip,
            raw_size,
            decoded: OnceLock::new(),
        }
    }

    pub(super) fn present(&self) -> bool {
        !self.gzip.is_empty()
    }

    pub(super) fn bytes(&self) -> Option<&[u8]> {
        if !self.present() {
            return None;
        }
        self.decoded
            .get_or_init(|| decode_object(self.gzip, self.raw_size.parse().ok()?))
            .as_deref()
    }
}

/// Read the raw length measured by build.rs. Read one more byte to check
/// gzip EOF and its checksum before publishing the bytes.
fn decode_object(gzip: &[u8], raw_size: usize) -> Option<Box<[u8]>> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(raw_size).ok()?;
    bytes.resize(raw_size, 0);
    let mut decoder = GzDecoder::new(gzip);
    decoder.read_exact(&mut bytes).ok()?;
    if decoder.read(&mut [0]).ok()? != 0 || !decoder.get_ref().is_empty() {
        return None;
    }
    Some(bytes.into_boxed_slice())
}

pub(super) static GNU: Embedded = Embedded::new(
    include_bytes!(concat!(env!("OUT_DIR"), "/interpose-gnu.so.gz")),
    env!("PODVM_INTERPOSE_GNU_SIZE"),
);
// TODO/interpose.md T-1880. This object needs no libdl.so.2.
pub(super) static GNU234: Embedded = Embedded::new(
    include_bytes!(concat!(env!("OUT_DIR"), "/interpose-gnu234.so.gz")),
    env!("PODVM_INTERPOSE_GNU234_SIZE"),
);
pub(super) static MUSL: Embedded = Embedded::new(
    include_bytes!(concat!(env!("OUT_DIR"), "/interpose-musl.so.gz")),
    env!("PODVM_INTERPOSE_MUSL_SIZE"),
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gzip_object_requires_exact_size_and_complete_member() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let raw = b"\x7fELF test bytes";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(raw).unwrap();
        let gzip = encoder.finish().unwrap();
        assert_eq!(
            decode_object(&gzip, raw.len()).as_deref(),
            Some(raw.as_slice())
        );
        assert!(decode_object(&gzip, raw.len() - 1).is_none());
        assert!(decode_object(&gzip, raw.len() + 1).is_none());
        assert!(decode_object(&gzip[..gzip.len() - 1], raw.len()).is_none());
        let mut bad_crc = gzip.clone();
        bad_crc[gzip.len() - 8] ^= 1;
        assert!(decode_object(&bad_crc, raw.len()).is_none());
        let mut trailing = gzip;
        trailing.push(0);
        assert!(decode_object(&trailing, raw.len()).is_none());
    }

    #[test]
    fn embedded_objects_match_reported_digests() {
        use crate::machine::guest::kernel::sha256_hex;

        for (embedded, digest) in [
            (&GNU, env!("PODVM_INTERPOSE_GNU")),
            (&GNU234, env!("PODVM_INTERPOSE_GNU234")),
            (&MUSL, env!("PODVM_INTERPOSE_MUSL")),
        ] {
            if digest == "absent" {
                assert!(!embedded.present());
                assert!(embedded.bytes().is_none());
            } else {
                let raw = embedded.bytes().expect("decoding embedded object");
                assert_eq!(sha256_hex(raw), digest);
            }
        }
    }
}
