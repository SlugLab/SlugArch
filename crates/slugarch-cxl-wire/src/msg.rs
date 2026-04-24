//! CXL message types. v1 covers .mem Req/RwD/DRS/NDR + .cache D2H/H2D
//! envelopes. All variants encodable/decodable; v1 host runtime only
//! actually produces M2SReq + M2SRwD and consumes S2MDRS + S2MNDR.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MsgClass {
    M2SReq = 0x1,
    M2SRwD = 0x2,
    S2MDRS = 0x3,
    S2MNDR = 0x4,
    D2HReq = 0x5,
    D2HResp = 0x6,
    H2DReq = 0x7,
    H2DResp = 0x8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum M2SReqOp {
    MemRd = 0x0,
    MemRdData = 0x1,
    MemInv = 0x2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum M2SRwDOp {
    MemWr = 0x0,
    MemWrPtl = 0x1,
    MemClnEvct = 0x2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum S2MDRSOp {
    MemData = 0x0,
    MemDataNxm = 0x1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum S2MNDROp {
    Cmp = 0x0,
    CmpS = 0x1,
    CmpE = 0x2,
    CmpI = 0x3,
    MemPassDirty = 0x4,
    DispatchFailed = 0xF,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum D2HReqOp {
    RdShared = 0x0,
    RdOwn = 0x1,
    Inval = 0x2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum D2HRespOp {
    RspIFwdM = 0x0,
    RspSFwdM = 0x1,
    RspV = 0x2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum H2DReqOp {
    SnpData = 0x0,
    SnpInv = 0x1,
    SnpCur = 0x2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum H2DRespOp {
    GoWritePull = 0x0,
    GoErr = 0x1,
    Go = 0x2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CxlMsg {
    M2SReq {
        tag: u16,
        opcode: M2SReqOp,
        addr: u64,
    },
    M2SRwD {
        tag: u16,
        opcode: M2SRwDOp,
        addr: u64,
        data: [u8; 32],
    },
    S2MDRS {
        tag: u16,
        opcode: S2MDRSOp,
        data: [u8; 32],
    },
    S2MNDR {
        tag: u16,
        opcode: S2MNDROp,
    },
    D2HReq {
        tag: u16,
        opcode: D2HReqOp,
        addr: u64,
    },
    D2HResp {
        tag: u16,
        opcode: D2HRespOp,
        data: Option<[u8; 32]>,
    },
    H2DReq {
        tag: u16,
        opcode: H2DReqOp,
        addr: u64,
    },
    H2DResp {
        tag: u16,
        opcode: H2DRespOp,
    },
}

impl CxlMsg {
    pub fn class(&self) -> MsgClass {
        match self {
            CxlMsg::M2SReq { .. } => MsgClass::M2SReq,
            CxlMsg::M2SRwD { .. } => MsgClass::M2SRwD,
            CxlMsg::S2MDRS { .. } => MsgClass::S2MDRS,
            CxlMsg::S2MNDR { .. } => MsgClass::S2MNDR,
            CxlMsg::D2HReq { .. } => MsgClass::D2HReq,
            CxlMsg::D2HResp { .. } => MsgClass::D2HResp,
            CxlMsg::H2DReq { .. } => MsgClass::H2DReq,
            CxlMsg::H2DResp { .. } => MsgClass::H2DResp,
        }
    }

    pub fn tag(&self) -> u16 {
        match self {
            CxlMsg::M2SReq { tag, .. }
            | CxlMsg::M2SRwD { tag, .. }
            | CxlMsg::S2MDRS { tag, .. }
            | CxlMsg::S2MNDR { tag, .. }
            | CxlMsg::D2HReq { tag, .. }
            | CxlMsg::D2HResp { tag, .. }
            | CxlMsg::H2DReq { tag, .. }
            | CxlMsg::H2DResp { tag, .. } => *tag,
        }
    }
}

impl MsgClass {
    pub(crate) fn from_nibble(n: u8) -> Option<Self> {
        Some(match n {
            0x1 => MsgClass::M2SReq,
            0x2 => MsgClass::M2SRwD,
            0x3 => MsgClass::S2MDRS,
            0x4 => MsgClass::S2MNDR,
            0x5 => MsgClass::D2HReq,
            0x6 => MsgClass::D2HResp,
            0x7 => MsgClass::H2DReq,
            0x8 => MsgClass::H2DResp,
            _ => return None,
        })
    }
}
