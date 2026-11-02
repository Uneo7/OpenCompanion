use oc_ais::eeprom::body;
use oc_ais::eeprom::graph::GraphData;
use oc_ais::eeprom::header::ImageHeader;
use oc_ais::eeprom::image::AppImage;
use oc_ais::eeprom::layout::{BOOT1, EE_SIZE, RAM_TRAMP, STOCK_IMAGE_END};
use oc_ais::{BuildOptions, Segment, MAGIC};
use oc_install::dsp::DspBundle;
use oc_install::tas::TasTarget;
use oc_ramload::LoadImage;
use oc_tas_image::{DataType, Header, Image, BOSE_HEADER};

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }
    pub fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n)
            .map(|_| {
                self.0 ^= self.0 << 13;
                self.0 ^= self.0 >> 7;
                self.0 ^= self.0 << 17;
                (self.0 >> 24) as u8
            })
            .collect()
    }
}

/// Stock-like 64 KB image: AIS (DisableCrc, a section over the loader area, one more section) ending exactly at
/// 0xDAD8, 0xFF up to 0xFFFE, 0x02 at 0xFFFF.
pub fn stock_dump(rng: &mut Rng) -> Vec<u8> {
    let loader = Segment { addr: 0x1002_3000, data: rng.bytes(0x1E00) };
    let rest_len = STOCK_IMAGE_END - 40 - 0x1E00;
    let other = Segment { addr: 0x1000_0000, data: rng.bytes(rest_len) };
    let img = oc_ais::build(&[loader, other], &[], 0x1002_C060, &BuildOptions::default()).unwrap();
    let mut out = oc_ais::write(&img).unwrap();
    assert_eq!(out.len(), STOCK_IMAGE_END, "synthetic stock image must end at 0xDAD8");
    out.resize(EE_SIZE, 0xFF);
    out[EE_SIZE - 1] = 0x02;
    out
}

pub fn boot0(rng: &mut Rng) -> [u8; 256] {
    let mut b = [0xFFu8; 256];
    let code = rng.bytes(8);
    let mut v = Vec::new();
    for w in [MAGIC, 0x5853_5901, RAM_TRAMP, code.len() as u32] {
        v.extend_from_slice(&w.to_le_bytes());
    }
    v.extend_from_slice(&code);
    for w in [0x5853_5906u32, RAM_TRAMP] {
        v.extend_from_slice(&w.to_le_bytes());
    }
    b[..v.len()].copy_from_slice(&v);
    b
}

pub fn boot1(rng: &mut Rng) -> Vec<u8> {
    let mut b = rng.bytes(BOOT1.len() - 4);
    let crc = crc32fast::hash(&b);
    b.extend_from_slice(&crc.to_le_bytes());
    b
}

pub fn app(rng: &mut Rng, fw_version: u32, code_len: usize) -> AppImage {
    let mut code = rng.bytes(code_len);
    code[100..1100].fill(0x11); // something compressible
    let segs = [Segment { addr: 0x1000_1000, data: code }];
    let body = body::build(&segs, &[(0x1000_9000, 4096)], 0x1000_1000).unwrap();
    let header = ImageHeader {
        seq: 0,
        body_len: body.bytes.len() as u32,
        body_crc: crc32fast::hash(&body.bytes),
        entry: body.entry,
        flags: 0,
        fw_version,
        raw_len: body.raw_len,
    };
    let graph = Some(GraphData::new(0x1002_7B2C, rng.bytes(921)).unwrap());
    AppImage { header, header_page: header.to_page(), body, graph }
}

pub fn bundle(seed: u64, fw_version: u32) -> DspBundle {
    let mut rng = Rng::new(0xC0FFEE); // stock / boot chain identical across bundles
    let stock = stock_dump(&mut rng);
    let stub = LoadImage { segments: vec![(0x1002_3000, rng.bytes(64))], fills: vec![], entry: 0x1002_3000 };
    let updater = LoadImage {
        segments: vec![(0x1000_1000, rng.bytes(300))],
        fills: vec![(0x1000_1200, 0x40, 0)],
        entry: 0x1000_1000,
    };
    let boot0 = boot0(&mut rng);
    let boot1 = boot1(&mut rng);
    let mut rng = Rng::new(seed);
    let app = app(&mut rng, fw_version, 3000);
    let b = DspBundle { stock, stub, updater, boot0, boot1, app };
    b.validate().unwrap();
    b
}

pub fn tas_bose(rng: &mut Rng) -> Vec<u8> {
    let mut ee = vec![0xFF; 8192];
    ee[..18].copy_from_slice(&BOSE_HEADER);
    ee[18..18 + 0x15E4].copy_from_slice(&rng.bytes(0x15E4));
    ee
}

/// OpenCompanion TAS image like `mkdfu.py` builds it (VID 05A7 PID 1020, attributes 0x08).
pub fn tas_target(seed: u64, code_len: usize, fw: u8) -> TasTarget {
    TasTarget::new(tas_image(seed, code_len, fw)).unwrap()
}

fn tas_image(seed: u64, code_len: usize, fw: u8) -> Image {
    let mut h = Header::bose_with(DataType::Application);
    h.product_version = 0x0A;
    h.firmware_version = fw;
    h.usb_attributes = 0x04;
    h.attributes = 0x08;
    Image::build(h, Rng::new(seed).bytes(code_len)).unwrap()
}

/// Data area like `mkeedata.py` builds it (layout 1, `len` payload bytes).
pub fn data_area(seed: u64, len: usize) -> oc_tas_image::data_area::DataArea {
    oc_tas_image::data_area::DataArea::new(1, Rng::new(seed).bytes(len)).unwrap()
}

pub fn tas_target_with_data(seed: u64, code_len: usize, fw: u8, data_len: usize) -> TasTarget {
    TasTarget::with_data(tas_image(seed, code_len, fw), data_area(seed + 100, data_len)).unwrap()
}
