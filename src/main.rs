use serde_synphasor::de::SynphasorDeserializer;
use serde_synphasor::frame::baseframe::*;
use serde_synphasor::frame::cfgframes::*;
use serde_synphasor::frame::commandframe::*;
use serde_synphasor::frame::dataframe::*;
use serde_synphasor::ser::SynphasorSerializer;
use serde_synphasor::synstream::*;
use std::time::SystemTime;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:4712").await.unwrap();

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        process(socket).await;
    }
}

async fn process(mut socket: TcpStream) {
    let mut buffer = [0; 1024];
    let pmu_dataset = new_pmu_dataset();
    let serializer = SynphasorSerializer::new(&pmu_dataset);
    let bytes = serializer.serialize_cfg_3_bytes().unwrap();
    loop {
        socket.read(&mut buffer).await.unwrap();
        let base_frame = SynphasorDeserializer::from_bytes(&buffer).unwrap();
        println!("{:?}", base_frame);
        match base_frame.frame {
            SynFrame::Cmd(v) => match v.command_type {
                SynCommandEnum::SendCfg3Frame => {
                    println!("Sending Config Frame");
                    socket.write_all(&bytes[..]).await;
                    println!("Sent Config Frame");
                }
                SynCommandEnum::TurnOnDataFrames => {
                    println!("Sending Data frames");
                    let one_cycle = Duration::from_millis(16);

                    //let soc = 0;
                    //let fracsec = 0;
                    while true {
                        let now = SystemTime::now();
                        let since_unix_epoch = now
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .expect("Time went backwards");
                        let in_ms = since_unix_epoch.as_millis();
                        let in_s = (in_ms / 1000) as u32;
                        let in_ms = in_ms - ((in_s as u128) * 1000);
                        let time: SynTime = SynTime {
                            soc: in_s,
                            fracsec: in_ms as u32,
                            leap_second_direction: false,
                            leap_second_occured: false,
                            leap_second_pending: false,
                            time_quality: SynTimeQuality::Locked,
                        };

                        let data_frame = new_data_frame();
                        let data_bytes = serializer.serialize_data_bytes(time, data_frame).unwrap();
                        socket.write(&data_bytes[..]).await;
                        sleep(Duration::from_millis(16)).await;
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

fn new_pmu_dataset() -> PMUDataSet {
    let stn = String::from("STN1");
    let idcode = 2;
    let g_pmu_id = [0; 16];
    let pmu_lat: Option<f32> = None;
    let pmu_lon: Option<f32> = None;
    let pmu_elev: Option<f32> = None;
    let svc_class = SynSVCClass::M;
    let window: i32 = 0;
    let grp_dly: i32 = 0;
    let fnom = SynNominalFreq::F60Hz;
    let data_rate = 0;
    let cfgcnt = 0;

    let analog_1: SynAnalogChannel =
        SynAnalogChannel::new(AnalogUnit::AnalogInputRMS, (1.0, 0.0), String::from("RTD1"));

    let digital_1 = DigitalFormat::new(false, true, String::from("52A"));

    let phasor_1 = SynPhasorChannel::new(
        PhasorUnit::Voltage,
        (1.0, 0.0),
        String::from("VA"),
        PhasorComponent::PhaseA,
        None,
    );

    let data_format = SynStreamFormat {
        freq_dfreq: SynAnalogType::AnalogFormatI16,
        //analogs: (SynAnalogType::AnalogFormatI16, vec![]),
        analogs: (SynAnalogType::AnalogFormatI16, vec![analog_1]),
        //phasors: (SynPhasorType::RectangularPhasorFormatI16, vec![]),
        phasors: (SynPhasorType::RectangularPhasorFormatI16, vec![phasor_1]),
        digitals: vec![digital_1],
        //digitals: vec![],
    };

    let test_stream = PMUStream {
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
        cfgcnt,
    };
    PMUDataSet::new(idcode, 1000, vec![test_stream], 60)
}

fn new_data_frame() -> SynDataFrame {
    let data = SynData::new(
        SynDataIndication::PMUDataGood,
        false,
        false,
        false,
        false,
        SynPMUTimeQuality::MTElt100ns,
        SynUnlockedTime::LockedOrUnlockedLT10s,
        SynTrigger::Reserved,
        //SynPhasorData::None,
        SynPhasorData::RectangularPhasorFormatI16(vec![(10i16, 10i16)]),
        SynFreqData::AnalogFormatI16(0),
        SynFreqData::AnalogFormatI16(0),
        SynAnalogData::AnalogFormatI16(vec![132]),
        vec![true],
    );
    SynDataFrame::new(vec![data])
}
