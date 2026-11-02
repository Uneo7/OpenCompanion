pub const ID: u8 = 1;
pub const NAME: &str = "c5-stock-1";
pub const SAMPLE_RATE: u32 = 48_000;
/// Delays are shorter than one 96-sample frame.
pub const DELAY_MAX: u8 = 95;

/// Payload order (spec 5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SetId {
    EqDirect,
    EqArray,
    EqSurround,
    EqBass,
    SurPre,
    SurStage1,
    SurStage2,
    SurStage3,
    FrontDirect,
    FrontArrayA,
    FrontArrayX,
    FrontStageB,
    FrontStageBX,
    FrontStageC,
    FrontStageCX,
    BassPhase,
}

/// Annotation only (spec 2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Tilt,
    Voicing,
    Cutoff,
    AllPass,
    Network,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::Tilt => "tilt",
            Role::Voicing => "voicing",
            Role::Cutoff => "cut-off",
            Role::AllPass => "all-pass",
            Role::Network => "network",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SetDef {
    pub id: SetId,
    pub block: &'static str,
    pub key: &'static str,
    pub nsec: usize,
    pub stock: u32,
    pub stock_set: usize,
    /// Annotation only.
    pub roles: &'static [Role],
}

use Role::{AllPass as A, Cutoff as C, Network as N, Tilt as T, Voicing as V};

const NET1: &[Role] = &[N];
const NET2: &[Role] = &[N, N];
const NET8: &[Role] = &[N; 8];

/// Payload order.
pub const SETS: [SetDef; 16] = [
    SetDef { id: SetId::EqDirect, block: "driver_eq", key: "direct", nsec: 8, stock: 0x1001_0884, stock_set: 0, roles: &[T, T, V, V, V, V, C, C] },
    SetDef { id: SetId::EqArray, block: "driver_eq", key: "array", nsec: 8, stock: 0x1001_0884, stock_set: 1, roles: &[V, V, V, V, V, C, C, C] },
    SetDef { id: SetId::EqSurround, block: "driver_eq", key: "surround", nsec: 8, stock: 0x1001_0884, stock_set: 2, roles: &[V, V, V, C, V, C, V, C] },
    SetDef { id: SetId::EqBass, block: "driver_eq", key: "bass", nsec: 8, stock: 0x1001_0884, stock_set: 3, roles: &[C, C, C, C, C, V, V, C] },
    SetDef { id: SetId::SurPre, block: "surround_net", key: "pre", nsec: 0, stock: 0x1001_0D80, stock_set: 0, roles: &[] },
    SetDef { id: SetId::SurStage1, block: "surround_net", key: "stage1", nsec: 8, stock: 0x1001_0E14, stock_set: 0, roles: NET8 },
    SetDef { id: SetId::SurStage2, block: "surround_net", key: "stage2", nsec: 8, stock: 0x1001_0EF8, stock_set: 0, roles: NET8 },
    SetDef { id: SetId::SurStage3, block: "surround_net", key: "stage3", nsec: 2, stock: 0x1001_0FDC, stock_set: 0, roles: NET2 },
    SetDef { id: SetId::FrontDirect, block: "front_net", key: "direct", nsec: 1, stock: 0x1001_1054, stock_set: 0, roles: NET1 },
    SetDef { id: SetId::FrontArrayA, block: "front_net", key: "array_a", nsec: 1, stock: 0x1001_1090, stock_set: 0, roles: NET1 },
    SetDef { id: SetId::FrontArrayX, block: "front_net", key: "array_x", nsec: 2, stock: 0x1001_111C, stock_set: 0, roles: NET2 },
    SetDef { id: SetId::FrontStageB, block: "front_net", key: "stage_b", nsec: 1, stock: 0x1001_1194, stock_set: 0, roles: NET1 },
    SetDef { id: SetId::FrontStageBX, block: "front_net", key: "stage_b_x", nsec: 1, stock: 0x1001_120C, stock_set: 0, roles: NET1 },
    SetDef { id: SetId::FrontStageC, block: "front_net", key: "stage_c", nsec: 2, stock: 0x1001_1158, stock_set: 0, roles: NET2 },
    SetDef { id: SetId::FrontStageCX, block: "front_net", key: "stage_c_x", nsec: 1, stock: 0x1001_11D0, stock_set: 0, roles: NET1 },
    SetDef { id: SetId::BassPhase, block: "bass_phase", key: "", nsec: 1, stock: 0x1001_08C0, stock_set: 0, roles: &[A] },
];

impl SetId {
    pub fn def(self) -> &'static SetDef {
        SETS.iter().find(|d| d.id == self).expect("every SetId is in SETS")
    }

    pub fn index(self) -> usize {
        SETS.iter().position(|d| d.id == self).expect("every SetId is in SETS")
    }
}

/// The eight delays, in payload order: (JSON block, key, element, stock delay module).
pub const DELAYS: [(&str, &str, usize, u32); 8] = [
    ("surround_net", "delay_in", 0, 0x1001_0DBC),
    ("surround_net", "delay_in", 1, 0x1001_0DE8),
    ("surround_net", "cross1", 0, 0x1001_0E50),
    ("surround_net", "cross1", 1, 0x1001_0E7C),
    ("surround_net", "cross2", 0, 0x1001_0F34),
    ("surround_net", "cross2", 1, 0x1001_0F60),
    ("front_net", "delay", 0, 0x1001_1248),
    ("front_net", "delay", 1, 0x1001_1274),
];

pub mod stock {
    /// `aux_mix`: mixers (usb, aux) of L and R.
    pub const AUX_MIX: [u32; 2] = [0x1001_04EC, 0x1001_0514];
    /// `input_gain`: gain module, 6 targets (L, C, R, LFE, Ls, Rs).
    pub const INPUT_GAIN: u32 = 0x1001_0850;
    /// `centre_mix`: mixers (centre, direct) of L and R.
    pub const CENTRE_MIX: [u32; 2] = [0x1001_0B80, 0x1001_0BA8];
    /// `surround_mix`: mixers (surround, third) of L and R.
    pub const SURROUND_MIX: [u32; 2] = [0x1001_0BD0, 0x1001_0BF8];
    /// `bass_sum`: sum module, 7 weights.
    pub const BASS_SUM: u32 = 0x1001_1430;
    /// Unity mixers inside the networks and the output summing: fixed (1, 1) in topology 1 [F].
    pub const UNITY_MIXERS: [u32; 16] = [
        0x1001_0EA8, 0x1001_0ED0, 0x1001_0F8C, 0x1001_0FB4, 0x1001_10CC, 0x1001_10F4, 0x1001_12F0, 0x1001_1340,
        0x1001_12A0, 0x1001_12C8, 0x1001_1318, 0x1001_1368, 0x1001_1390, 0x1001_13B8, 0x1001_13E0, 0x1001_1408,
    ];
    /// Channel → coefficient set of the stock driver EQ (direct L/R, array L/R, surround L/R, bass).
    pub const DRIVER_EQ_CHANNELS: [usize; 7] = [0, 0, 1, 1, 2, 2, 3];
}

/// Scalar floats of the payload: aux 4, input gain 6, centre 4, surround 4, bass sum 7.
pub const SCALARS: usize = 25;

pub fn payload_floats() -> usize {
    SCALARS + SETS.iter().map(|d| 1 + 4 * d.nsec).sum::<usize>()
}

pub fn payload_len() -> usize {
    4 * payload_floats() + DELAYS.len()
}
