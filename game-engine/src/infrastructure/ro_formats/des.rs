#[repr(C)]
#[derive(Copy, Clone)]
pub union Bit64 {
    pub b: [u8; 8],
}

impl Default for Bit64 {
    fn default() -> Self {
        Bit64 { b: [0; 8] }
    }
}

const MASK: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];

fn ip(src: &mut Bit64) {
    let mut tmp = Bit64::default();

    const IP_TABLE: [u8; 64] = [
        58, 50, 42, 34, 26, 18, 10, 2, 60, 52, 44, 36, 28, 20, 12, 4, 62, 54, 46, 38, 30, 22, 14,
        6, 64, 56, 48, 40, 32, 24, 16, 8, 57, 49, 41, 33, 25, 17, 9, 1, 59, 51, 43, 35, 27, 19, 11,
        3, 61, 53, 45, 37, 29, 21, 13, 5, 63, 55, 47, 39, 31, 23, 15, 7,
    ];

    unsafe {
        for i in 0..IP_TABLE.len() {
            let j = (IP_TABLE[i] - 1) as usize;
            if src.b[(j >> 3) & 7] & MASK[j & 7] != 0 {
                tmp.b[(i >> 3) & 7] |= MASK[i & 7];
            }
        }
        *src = tmp;
    }
}

fn fp(src: &mut Bit64) {
    let mut tmp = Bit64::default();

    const FP_TABLE: [u8; 64] = [
        40, 8, 48, 16, 56, 24, 64, 32, 39, 7, 47, 15, 55, 23, 63, 31, 38, 6, 46, 14, 54, 22, 62,
        30, 37, 5, 45, 13, 53, 21, 61, 29, 36, 4, 44, 12, 52, 20, 60, 28, 35, 3, 43, 11, 51, 19,
        59, 27, 34, 2, 42, 10, 50, 18, 58, 26, 33, 1, 41, 9, 49, 17, 57, 25,
    ];

    unsafe {
        for i in 0..FP_TABLE.len() {
            let j = (FP_TABLE[i] - 1) as usize;
            if src.b[(j >> 3) & 7] & MASK[j & 7] != 0 {
                tmp.b[(i >> 3) & 7] |= MASK[i & 7];
            }
        }
        *src = tmp;
    }
}

fn e(src: &mut Bit64) {
    let mut tmp = Bit64::default();

    unsafe {
        tmp.b[0] = ((src.b[7] << 5) | (src.b[4] >> 3)) & 0x3f;
        tmp.b[1] = ((src.b[4] << 1) | (src.b[5] >> 7)) & 0x3f;
        tmp.b[2] = ((src.b[4] << 5) | (src.b[5] >> 3)) & 0x3f;
        tmp.b[3] = ((src.b[5] << 1) | (src.b[6] >> 7)) & 0x3f;
        tmp.b[4] = ((src.b[5] << 5) | (src.b[6] >> 3)) & 0x3f;
        tmp.b[5] = ((src.b[6] << 1) | (src.b[7] >> 7)) & 0x3f;
        tmp.b[6] = ((src.b[6] << 5) | (src.b[7] >> 3)) & 0x3f;
        tmp.b[7] = ((src.b[7] << 1) | (src.b[4] >> 7)) & 0x3f;

        *src = tmp;
    }
}

fn tp(src: &mut Bit64) {
    let mut tmp = Bit64::default();

    const TP_TABLE: [u8; 32] = [
        16, 7, 20, 21, 29, 12, 28, 17, 1, 15, 23, 26, 5, 18, 31, 10, 2, 8, 24, 14, 32, 27, 3, 9,
        19, 13, 30, 6, 22, 11, 4, 25,
    ];

    unsafe {
        for i in 0..TP_TABLE.len() {
            let j = (TP_TABLE[i] - 1) as usize;
            if src.b[j >> 3] & MASK[j & 7] != 0 {
                tmp.b[(i >> 3) + 4] |= MASK[i & 7];
            }
        }
        *src = tmp;
    }
}

fn sbox(src: &mut Bit64) {
    let mut tmp = Bit64::default();

    const S_TABLE: [[u8; 64]; 4] = [
        [
            0xef, 0x03, 0x41, 0xfd, 0xd8, 0x74, 0x1e, 0x47, 0x26, 0xef, 0xfb, 0x22, 0xb3, 0xd8,
            0x84, 0x1e, 0x39, 0xac, 0xa7, 0x60, 0x62, 0xc1, 0xcd, 0xba, 0x5c, 0x96, 0x90, 0x59,
            0x05, 0x3b, 0x7a, 0x85, 0x40, 0xfd, 0x1e, 0xc8, 0xe7, 0x8a, 0x8b, 0x21, 0xda, 0x43,
            0x64, 0x9f, 0x2d, 0x14, 0xb1, 0x72, 0xf5, 0x5b, 0xc8, 0xb6, 0x9c, 0x37, 0x76, 0xec,
            0x39, 0xa0, 0xa3, 0x05, 0x52, 0x6e, 0x0f, 0xd9,
        ],
        [
            0xa7, 0xdd, 0x0d, 0x78, 0x9e, 0x0b, 0xe3, 0x95, 0x60, 0x36, 0x36, 0x4f, 0xf9, 0x60,
            0x5a, 0xa3, 0x11, 0x24, 0xd2, 0x87, 0xc8, 0x52, 0x75, 0xec, 0xbb, 0xc1, 0x4c, 0xba,
            0x24, 0xfe, 0x8f, 0x19, 0xda, 0x13, 0x66, 0xaf, 0x49, 0xd0, 0x90, 0x06, 0x8c, 0x6a,
            0xfb, 0x91, 0x37, 0x8d, 0x0d, 0x78, 0xbf, 0x49, 0x11, 0xf4, 0x23, 0xe5, 0xce, 0x3b,
            0x55, 0xbc, 0xa2, 0x57, 0xe8, 0x22, 0x74, 0xce,
        ],
        [
            0x2c, 0xea, 0xc1, 0xbf, 0x4a, 0x24, 0x1f, 0xc2, 0x79, 0x47, 0xa2, 0x7c, 0xb6, 0xd9,
            0x68, 0x15, 0x80, 0x56, 0x5d, 0x01, 0x33, 0xfd, 0xf4, 0xae, 0xde, 0x30, 0x07, 0x9b,
            0xe5, 0x83, 0x9b, 0x68, 0x49, 0xb4, 0x2e, 0x83, 0x1f, 0xc2, 0xb5, 0x7c, 0xa2, 0x19,
            0xd8, 0xe5, 0x7c, 0x2f, 0x83, 0xda, 0xf7, 0x6b, 0x90, 0xfe, 0xc4, 0x01, 0x5a, 0x97,
            0x61, 0xa6, 0x3d, 0x40, 0x0b, 0x58, 0xe6, 0x3d,
        ],
        [
            0x4d, 0xd1, 0xb2, 0x0f, 0x28, 0xbd, 0xe4, 0x78, 0xf6, 0x4a, 0x0f, 0x93, 0x8b, 0x17,
            0xd1, 0xa4, 0x3a, 0xec, 0xc9, 0x35, 0x93, 0x56, 0x7e, 0xcb, 0x55, 0x20, 0xa0, 0xfe,
            0x6c, 0x89, 0x17, 0x62, 0x17, 0x62, 0x4b, 0xb1, 0xb4, 0xde, 0xd1, 0x87, 0xc9, 0x14,
            0x3c, 0x4a, 0x7e, 0xa8, 0xe2, 0x7d, 0xa0, 0x9f, 0xf6, 0x5c, 0x6a, 0x09, 0x8d, 0xf0,
            0x0f, 0xe3, 0x53, 0x25, 0x95, 0x36, 0x28, 0xcb,
        ],
    ];

    unsafe {
        (0..S_TABLE.len()).for_each(|i| {
            tmp.b[i] = (S_TABLE[i][src.b[i * 2] as usize] & 0xf0)
                | (S_TABLE[i][src.b[i * 2 + 1] as usize] & 0x0f);
        });
        *src = tmp;
    }
}

fn round_function(src: &mut Bit64) {
    let mut tmp = *src;
    e(&mut tmp);
    sbox(&mut tmp);
    tp(&mut tmp);

    unsafe {
        src.b[0] ^= tmp.b[4];
        src.b[1] ^= tmp.b[5];
        src.b[2] ^= tmp.b[6];
        src.b[3] ^= tmp.b[7];
    }
}

pub fn des_decrypt_block(block: &mut Bit64) {
    ip(block);
    round_function(block);
    fp(block);
}

// Shuffle decode table for GRF decryption
const SHUFFLE_TABLE: [u8; 256] = {
    let mut table = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        table[i] = i as u8;
        i += 1;
    }
    // Swap pairs as defined in roBrowser
    let swaps = [
        (0x00, 0x2b),
        (0x6c, 0x80),
        (0x01, 0x68),
        (0x48, 0x77),
        (0x60, 0xff),
        (0xb9, 0xc0),
        (0xfe, 0xeb),
    ];
    let mut j = 0;
    while j < swaps.len() {
        let (a, b) = swaps[j];
        table[a as usize] = b;
        table[b as usize] = a;
        j += 1;
    }
    table
};

fn shuffle_decode(data: &mut [u8], offset: usize) {
    if offset + 8 <= data.len() {
        let mut tmp = [0u8; 8];
        tmp[0] = data[offset + 3];
        tmp[1] = data[offset + 4];
        tmp[2] = data[offset + 6];
        tmp[3] = data[offset];
        tmp[4] = data[offset + 1];
        tmp[5] = data[offset + 2];
        tmp[6] = data[offset + 5];
        tmp[7] = SHUFFLE_TABLE[data[offset + 7] as usize];

        (0..8).for_each(|i| {
            data[offset + i] = tmp[i];
        });
    }
}

fn decrypt_block(data: &mut [u8], offset: usize) {
    if offset + 8 <= data.len() {
        let block = unsafe { &mut *(data[offset..].as_mut_ptr() as *mut Bit64) };
        des_decrypt_block(block);
    }
}

/// Decode the whole file with full DES encryption
pub fn decode_full(data: &mut [u8], length_aligned: u32, pack_size: u32) {
    let len = length_aligned as usize;
    let nblocks = len >> 3;

    // Compute number of digits of the entry length (RoBrowser uses pack_size!)
    let digits = pack_size.to_string().len();

    // Choose size of gap between two encrypted blocks
    // digits:  0  1  2  3  4  5  6  7  8  9 ...
    //  cycle:  1  1  1  4  5 14 15 22 23 24 ...
    let cycle = if digits < 3 {
        1
    } else if digits < 5 {
        digits + 1
    } else if digits < 7 {
        digits + 9
    } else {
        digits + 15
    };
    // First 20 blocks are all des-encrypted
    for i in 0..20.min(nblocks) {
        decrypt_block(data, i * 8);
    }

    // After block 20, apply pattern of DES and shuffle
    let mut j = 0;
    for i in 20..nblocks {
        // Decrypt block at cycle intervals
        if i % cycle == 0 {
            decrypt_block(data, i * 8);
            continue;
        }

        // De-shuffle block every 8th non-decrypted block
        if j == 7 {
            shuffle_decode(data, i * 8);
            j = 0;
        }

        j += 1;
    }
}

/// Decode only the header with DES encryption
pub fn decode_header(data: &mut [u8], length_aligned: u32) {
    let len = length_aligned as usize;
    let nblocks = len >> 3;

    // First 20 blocks are all des-encrypted
    for i in 0..20.min(nblocks) {
        decrypt_block(data, i * 8);
    }
    // The rest is plaintext, done.
}
