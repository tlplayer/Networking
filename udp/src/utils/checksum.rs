/// Internet checksum compatible with the original C lab helper.
///
/// The return value is normalized so a computed zero checksum is represented
/// as `0xffff`, matching `rlib.c`.
pub fn internet_checksum(bytes: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut chunks = bytes.chunks_exact(2);

    for chunk in &mut chunks {
        sum += u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
    }

    if let Some(&last) = chunks.remainder().first() {
        sum += (last as u32) << 8;
    }

    while sum > 0xffff {
        sum = (sum >> 16) + (sum & 0xffff);
    }

    let checksum = !(sum as u16);
    if checksum == 0 { 0xffff } else { checksum }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_checksum_is_all_ones() {
        assert_eq!(internet_checksum(&[]), 0xffff);
    }

    #[test]
    fn checksum_verifies_to_all_ones() {
        let mut bytes = [0u8; 8];
        bytes[2..4].copy_from_slice(&8u16.to_be_bytes());
        bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
        let checksum = internet_checksum(&bytes);
        bytes[0..2].copy_from_slice(&checksum.to_be_bytes());
        assert_eq!(internet_checksum(&bytes), 0xffff);
    }
}
