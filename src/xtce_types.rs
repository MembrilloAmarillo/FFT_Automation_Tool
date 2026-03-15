use crate::error::{Result, YamcsTcError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u16)]
pub enum FunctionId {
    UcfEpsOutputBusChannelOn = 516,
    UcfEpsOutputBusChannelOff = 517,
    UcfSystemChangeTime = 774,
    UcfEpsCorrectTime = 533,
    UcfPay1StopTime = 6,
    UcfPay2StopTime = 262,
    UcfPay1DownloadPacket = 7,
    UcfPay2DownloadPacket = 263,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u16)]
pub enum HkStructureId {
    Adcs = 1,
    EpsPbu = 2,
    EpsPdu = 3,
    EpsPiu = 4,
    EpsSys = 5,
    SolarPanels = 6,
    Transceiver = 7,
    EpsPcu = 8,
    EpsPiuOcf = 9,
    EpsPbuAbf = 10,
    EpsParam = 11,
    EpsObcMode = 12,
    AdcsSm = 17,
    EpsPbuSm = 18,
    EpsPduSm = 19,
    EpsPiuSm = 20,
    EpsSysSm = 21,
    AdcsNm = 33,
    EpsPbuNm = 34,
    EpsPduNm = 35,
    EpsPiuNm = 36,
    EpsSysNm = 37,
    AdcsOm = 49,
    EpsPbuOm = 50,
    EpsPduOm = 51,
    EpsPiuOm = 52,
    EpsSysOm = 53,
}

impl HkStructureId {
    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pus331Entry {
    #[serde(rename = "HK_Parameter_Report_Structure_ID")]
    pub hk_parameter_report_structure_id: u16,
    #[serde(rename = "Collection_Interval")]
    pub collection_interval: u32,
}

impl Pus331Entry {
    pub fn new(hk: HkStructureId, interval: u32) -> Self {
        Self {
            hk_parameter_report_structure_id: hk.as_u16(),
            collection_interval: interval,
        }
    }
}

pub fn ensure_count_matches<T>(n: usize, items: &[T], field: &str) -> Result<()> {
    if n != items.len() {
        return Err(YamcsTcError::Validation(format!(
            "{} count {} does not match provided items {}",
            field,
            n,
            items.len()
        )));
    }
    Ok(())
}

pub fn u24(value: u32) -> Result<u32> {
    if value > 0x00FF_FFFF {
        return Err(YamcsTcError::Validation(format!(
            "value {} exceeds u24 range",
            value
        )));
    }
    Ok(value)
}

pub fn hk_array_value(items: &[HkStructureId]) -> Value {
    Value::Array(items.iter().map(|i| json!(*i as u16)).collect())
}

pub fn u16_array_value(items: &[u16]) -> Value {
    Value::Array(items.iter().map(|i| json!(*i)).collect())
}

pub fn pus331_array_value(items: &[Pus331Entry]) -> Value {
    Value::Array(items.iter().map(|i| json!(i)).collect())
}
