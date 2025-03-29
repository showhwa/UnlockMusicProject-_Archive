use crate::v2_rc4::hash::hash;
use crate::v2_rc4::rc4::RC4;
use crate::v2_rc4::segment_key::get_segment_key;
use std::cmp::min;

const FIRST_SEGMENT_SIZE: usize = 0x0080;
const OTHER_SEGMENT_SIZE: usize = 0x1400;
const RC4_STREAM_CACHE_SIZE: usize = OTHER_SEGMENT_SIZE + 512;

#[derive(Debug, PartialEq, Clone)]
pub struct QMC2RC4 {
    hash: f64,
    key: Box<[u8]>,
    key_stream: Box<[u8; RC4_STREAM_CACHE_SIZE]>,
}

impl QMC2RC4 {
    pub fn new(key: &[u8]) -> Self {
        let mut rc4 = RC4::new(key);
        let mut key_stream = Box::new([0u8; RC4_STREAM_CACHE_SIZE]);
        rc4.derive(&mut key_stream[..]);

        Self {
            hash: hash(key),
            key: key.into(),
            key_stream,
        }
    }

    fn process_first_segment(&self, data: &mut [u8], offset: usize) {
        let n = self.key.len();

        for (datum, offset) in data.iter_mut().zip(offset..) {
            let idx = get_segment_key(offset as u64, self.key[offset % n], self.hash);
            let idx = idx % (n as u64);
            *datum ^= self.key[idx as usize];
        }
    }

    fn process_other_segment(&self, data: &mut [u8], offset: usize) {
        let n = self.key.len();

        let id = offset / OTHER_SEGMENT_SIZE;
        let block_offset = offset % OTHER_SEGMENT_SIZE;

        let seed = self.key[id % n];
        let skip = get_segment_key(id as u64, seed, self.hash);
        let skip = (skip & 0x1FF) as usize;

        debug_assert!(data.len() <= OTHER_SEGMENT_SIZE - block_offset);
        let key_stream = self.key_stream.iter().skip(skip + block_offset);
        for (datum, &key) in data.iter_mut().zip(key_stream) {
            *datum ^= key;
        }
    }

    pub fn decrypt<T>(&self, data: &mut T, offset: usize)
    where
        T: AsMut<[u8]> + ?Sized,
    {
        let mut offset = offset;
        let mut buffer = data.as_mut();
        if offset < FIRST_SEGMENT_SIZE {
            let n = min(FIRST_SEGMENT_SIZE - offset, buffer.len());
            let (block, rest) = buffer.split_at_mut(n);
            buffer = rest;
            self.process_first_segment(block, offset);
            offset += n;
        }

        match offset % OTHER_SEGMENT_SIZE {
            0 => {} // we are already in the boundary, nothing to do.
            excess => {
                let n = min(OTHER_SEGMENT_SIZE - excess, buffer.len());
                let (block, rest) = buffer.split_at_mut(n);
                buffer = rest;
                self.process_other_segment(block, offset);
                offset += n;
            }
        };

        while !buffer.is_empty() {
            let n = min(OTHER_SEGMENT_SIZE, buffer.len());
            let (block, rest) = buffer.split_at_mut(n);
            buffer = rest;
            self.process_other_segment(block, offset);
            offset += n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qmc2_header() {
        let mut data = [
            0x39, 0x5a, 0x4f, 0x75, 0x38, 0x71, 0x37, 0x6b, 0x36, 0x51, 0x53, 0x6d, 0x7a, 0x66,
            0x53, 0x4b, 0x66, 0x50, 0x69, 0x34, 0x67, 0x6c, 0x33, 0x7a, 0x55, 0x62, 0x35, 0x5a,
            0x32, 0x75, 0x4f, 0x68, 0x44, 0x52, 0x6d, 0x65, 0x75, 0x6e, 0x39, 0x52, 0x30, 0x7a,
            0x68, 0x62, 0x73, 0x59, 0x39, 0x48, 0x55, 0x57, 0x73, 0x32, 0x5a, 0x70, 0x64, 0x50,
            0x4e, 0x52, 0x6a, 0x63, 0x4d, 0x39, 0x37, 0x76, 0x72, 0x47, 0x64, 0x4d, 0x62, 0x6d,
            0x58, 0x68, 0x75, 0x47, 0x37, 0x56, 0x69, 0x6b, 0x4a, 0x79, 0x66, 0x63, 0x70, 0x39,
            0x59, 0x34, 0x43, 0x6b, 0x45, 0x32, 0x5a, 0x31, 0x38, 0x77, 0x70, 0x43, 0x51, 0x79,
            0x6a, 0x62, 0x32, 0x33, 0x65, 0x58, 0x4a, 0x4d, 0x33, 0x4e, 0x70, 0x62, 0x62, 0x67,
            0x4c, 0x54, 0x78, 0x64, 0x64, 0x77, 0x6e, 0x72, 0x37, 0x41, 0x54, 0x39, 0x42, 0x52,
            0x47, 0x32, 0x1a, 0xe4, 0x1b, 0x71, 0x68, 0x29, 0xb3, 0x6e, 0xad, 0xc5, 0x28, 0x12,
            0xd6, 0xa4, 0x4b, 0x06, 0x7a, 0xdc, 0x90, 0x15, 0x99, 0xd6, 0xbf, 0x72, 0xa2, 0x30,
            0x37, 0x6b, 0x5c, 0xd6, 0x2f, 0x35, 0x14, 0x8a, 0xd6, 0xfb, 0x9f, 0xee, 0x7d, 0x2d,
            0xb7, 0x37, 0xf2, 0x0b, 0x6e, 0x00, 0xfb, 0xa0, 0x3c, 0x40, 0xf3, 0x36, 0xb2, 0x76,
            0x20, 0x0f, 0x9e, 0xa5, 0xa3, 0x15, 0x60, 0x23, 0x15, 0x29, 0xa1, 0x91, 0xbf, 0xfb,
            0x12, 0x95, 0xaa, 0x8d, 0x92, 0xc6, 0x0b, 0x8d, 0x49, 0x99, 0xa5, 0xe0, 0x05, 0xcf,
            0xb6, 0xac, 0x07, 0x54, 0x58, 0x28, 0xf9, 0x96, 0xd1, 0x9a, 0xfe, 0x0b, 0x3c, 0xfb,
            0x0b, 0x25, 0x7a, 0x43, 0x5a, 0x33, 0xc3, 0x7a, 0xfc, 0x33, 0xa3, 0xc2, 0x65, 0x48,
            0x29, 0x8d, 0x2c, 0x8f, 0x4e, 0x88, 0xfd, 0x44, 0xfd, 0xd5, 0xca, 0xb9, 0x8d, 0x62,
            0x4a, 0x48, 0x20, 0x1du8,
        ];
        let key = (b'a'..=b'z')
            .chain(b'A'..=b'Z')
            .chain(b'0'..=b'9')
            .cycle()
            .take(512)
            .collect::<Vec<u8>>();

        let cipher = QMC2RC4::new(&key);
        cipher.decrypt(&mut data, 0);
        assert_eq!(data, [0u8; 256]);
    }
}
