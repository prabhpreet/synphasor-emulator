use serde_synphasor::{
    frame::baseframe::*, frame::dataframe::*, ser::SynphasorSerializer, synstream::*,
};
use std::time::SystemTime;

pub struct PMUData {
    pub va_m: f32,
    pub va_a: f32,
    pub vb_m: f32,
    pub vb_a: f32,
    pub vc_m: f32,
    pub vc_a: f32,
    pub ia_m: f32,
    pub ia_a: f32,
    pub ib_m: f32,
    pub ib_a: f32,
    pub ic_m: f32,
    pub ic_a: f32,
    pub freq: f32,
    pub df_dt: f32,
}

pub struct PMU {
    calculation: fn(SystemTime) -> PMUData,
    serializer: SynphasorSerializer,
    pub idcode: u16,
    pub stn: String,
    pub port: u16,
    pub data_rate: i16,
}

impl PMU {
    pub fn new(
        stn: String,
        idcode: u16,
        g_pmu_id: [u8; 16],
        pmu_lat: Option<f32>,
        pmu_lon: Option<f32>,
        pmu_elev: Option<f32>,
        svc_class: SynSVCClass,
        window: i32,
        grp_dly: i32,
        fnom: SynNominalFreq,
        data_rate: i16,
        calculation: fn(SystemTime) -> PMUData,
        port: u16,
    ) -> PMU {
        let phasor_va = SynPhasorChannel::new(
            PhasorUnit::Voltage,
            (1.0, 0.0),
            String::from("VA"),
            PhasorComponent::PhaseA,
            None,
        );

        let phasor_vb = SynPhasorChannel::new(
            PhasorUnit::Voltage,
            (1.0, 0.0),
            String::from("VB"),
            PhasorComponent::PhaseB,
            None,
        );

        let phasor_vc = SynPhasorChannel::new(
            PhasorUnit::Voltage,
            (1.0, 0.0),
            String::from("VC"),
            PhasorComponent::PhaseC,
            None,
        );

        let phasor_ia = SynPhasorChannel::new(
            PhasorUnit::Current,
            (1.0, 0.0),
            String::from("IA"),
            PhasorComponent::PhaseA,
            None,
        );

        let phasor_ib = SynPhasorChannel::new(
            PhasorUnit::Current,
            (1.0, 0.0),
            String::from("IB"),
            PhasorComponent::PhaseB,
            None,
        );

        let phasor_ic = SynPhasorChannel::new(
            PhasorUnit::Current,
            (1.0, 0.0),
            String::from("IC"),
            PhasorComponent::PhaseC,
            None,
        );

        let data_format = SynStreamFormat {
            freq_dfreq: SynAnalogType::AnalogFormatF32,
            analogs: (SynAnalogType::AnalogFormatF32, vec![]),
            phasors: (
                SynPhasorType::PolarPhasorFormatF32,
                vec![
                    phasor_va, phasor_vb, phasor_vc, phasor_ia, phasor_ib, phasor_ic,
                ],
            ),
            digitals: vec![],
        };
        let stn_clone = stn.clone();

        let pmu_stream = PMUStream::new(
            stn,
            idcode,
            g_pmu_id,
            data_format,
            pmu_lat,
            pmu_lon,
            pmu_elev,
            svc_class,
            window,
            grp_dly,
            fnom,
            0u16,
        );

        let pmu_data_set = PMUDataSet::new(idcode, 1000000, vec![pmu_stream], data_rate);

        let serializer = SynphasorSerializer::new(pmu_data_set);

        PMU {
            calculation,
            serializer,
            idcode,
            stn: stn_clone,
            port,
            data_rate,
        }
    }

    pub fn generate_data_frame(&self, time: SystemTime) -> Vec<u8> {
        let v = (self.calculation)(time);
        let data = SynData::new(
            SynDataIndication::PMUDataGood,
            false,
            false,
            false,
            false,
            SynPMUTimeQuality::MTElt100ns,
            SynUnlockedTime::LockedOrUnlockedLT10s,
            SynTrigger::Reserved,
            SynPhasorData::PolarPhasorFormatF32(vec![
                (v.va_m, v.va_a),
                (v.vb_m, v.vb_a),
                (v.vc_m, v.vc_a),
                (v.ia_m, v.ia_a),
                (v.ib_m, v.ib_a),
                (v.ic_m, v.ic_a),
            ]),
            SynFreqData::AnalogFormatF32(v.freq),
            SynFreqData::AnalogFormatF32(v.df_dt),
            SynAnalogData::None,
            vec![],
        );
        let since_unix_epoch = time
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time went backwards");
        let in_us = since_unix_epoch.as_micros();
        let in_s = (in_us / 1000000) as u32;
        let in_us = in_us - ((in_s as u128) * 1000000);
        let time: SynTime = SynTime {
            soc: in_s,
            fracsec: (in_us as u32),
            leap_second_direction: false,
            leap_second_occured: false,
            leap_second_pending: false,
            time_quality: SynTimeQuality::Locked,
        };

        let data_frame = SynDataFrame::new(vec![data]);
        self.serializer
            .serialize_data_bytes(time, data_frame)
            .unwrap()
    }
    pub fn get_cfg_frame(&self) -> Vec<u8> {
        self.serializer.serialize_cfg_3_bytes().unwrap()
    }
}
