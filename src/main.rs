use serde_synphasor::de::SynphasorDeserializer;
use serde_synphasor::frame::baseframe::*;
use serde_synphasor::frame::cfgframes::*;
use serde_synphasor::frame::commandframe::*;
use serde_synphasor::frame::dataframe::*;
use serde_synphasor::ser::SynphasorSerializer;
use serde_synphasor::synstream::*;
use serde_synphasor::SynError;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::net::SocketAddr;
use std::sync::mpsc::Receiver;
use std::time::SystemTime;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
mod psmodel;
use crate::psmodel::*;
use std::sync::Arc;
use tokio::sync::Mutex;

type TxStore = Arc<Mutex<HashMap<(u16, SocketAddr), tokio::sync::mpsc::Sender<WriteTask>>>>;
#[tokio::main]
async fn main() {
    let pmus = pmu_model();

    let tx_store: TxStore = Arc::new(Mutex::new(HashMap::new()));

    let pmu_data: Vec<(u16, u16, Arc<Vec<u8>>)> = pmus
        .iter()
        .map(|pmu| (pmu.idcode, pmu.port, Arc::new(pmu.get_cfg_frame())))
        .collect();

    let tx_store_publish = tx_store.clone();

    let feeder_publish_handle = tokio::spawn(async move {
        feeder_publish(tx_store_publish, pmus).await;
    });

    for (idcode, port, cfg_frame) in pmu_data.into_iter() {
        let tx_store_publish = tx_store.clone();
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))
            .await
            .unwrap();
        let cfg_frame = cfg_frame.clone();
        tokio::spawn(async move {
            listen(listener, tx_store_publish, idcode, cfg_frame).await;
        });
    }
    feeder_publish_handle.await.unwrap();
}

async fn listen(listener: TcpListener, tx_store: TxStore, idcode: u16, cfg_frame: Arc<Vec<u8>>) {
    loop {
        let tx_store_publish = tx_store.clone();
        let cfg_frame_clone = cfg_frame.clone();
        let (socket, _) = listener.accept().await.unwrap();
        let peer_addr = socket.peer_addr().unwrap();
        let (mut rd, mut wr) = io::split(socket);
        let (tx_channel, mut rx_channel) = mpsc::channel::<WriteTask>(120);

        tokio::spawn(async move {
            read_stream(rd, tx_store_publish, tx_channel, idcode, peer_addr).await;
        });

        tokio::spawn(async move {
            write_stream(wr, rx_channel, cfg_frame_clone).await;
        });
    }
}

enum WriteTask {
    SendCfg3Frame,
    SendDataFrame(Vec<u8>),
}

async fn write_stream(
    mut wr: WriteHalf<TcpStream>,
    mut rx_channel: mpsc::Receiver<WriteTask>,
    cfg_frame: Arc<Vec<u8>>,
) {
    while let Some(message) = rx_channel.recv().await {
        match message {
            WriteTask::SendCfg3Frame => {
                wr.write_all(&cfg_frame[..]).await;
            }
            WriteTask::SendDataFrame(v) => {
                wr.write_all(&v[..]).await;
            }
            _ => {}
        }
    }
}

async fn read_stream(
    mut rd: ReadHalf<TcpStream>,
    tx_store: TxStore,
    tx_channel: mpsc::Sender<WriteTask>,
    idcode: u16,
    peer_addr: SocketAddr,
) -> Result<(), SynError> {
    let mut buffer = [0; 4096];
    loop {
        rd.read(&mut buffer).await.unwrap();
        let base_frame = SynphasorDeserializer::from_bytes(&buffer)?;
        if (base_frame.idcode == idcode) {
            match base_frame.frame {
                SynFrame::Cmd(v) => match v.command_type {
                    SynCommandEnum::SendCfg3Frame => {
                        tx_channel.send(WriteTask::SendCfg3Frame).await;
                    }
                    SynCommandEnum::TurnOnDataFrames => {
                        let mut tx_store = tx_store.lock().await;
                        tx_store.insert((idcode, peer_addr), tx_channel.clone());
                    }
                    SynCommandEnum::TurnOffDataFrames => {
                        let mut tx_store = tx_store.lock().await;
                        tx_store.remove(&(idcode, peer_addr));
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

async fn feeder_publish(tx_store: TxStore, pmus: Vec<PMU>) {
    //Publish feeder information to all in Tx
    let mut time = SystemTime::now();
    //TODO: Base this on data_rate
    let mut interval = interval(Duration::from_micros(8333));
    let tx_store_publish = tx_store.clone();
    loop {
        {
            time = time.checked_add(Duration::from_micros(8333)).unwrap();
            let mut tx_store_publish = tx_store_publish.lock().await;

            let pmu_data_frames: HashMap<u16, Vec<u8>> = pmus
                .iter()
                .map(|pmu| (pmu.idcode, pmu.generate_data_frame(time)))
                .collect();

            for ((idcode, peer_addr), tx) in tx_store_publish.iter() {
                let data = pmu_data_frames.get(idcode).unwrap();
                tx.send(WriteTask::SendDataFrame(data.clone())).await;
            }
        }
        interval.tick().await;
    }
}

fn pmu_model() -> Vec<PMU> {
    let calculation = |t: SystemTime| PMUData {
        va_m: 120.,
        va_a: 0.,
        vb_m: 120.,
        vb_a: -2. * PI / 3.,
        vc_m: 120.,
        vc_a: 2. * PI / 3.,
        ia_m: 5.,
        ia_a: 0.,
        ib_m: 5.,
        ib_a: -2. * PI / 3.,
        ic_m: 5.,
        ic_a: 2. * PI / 3.,
        freq: 60.,
        df_dt: 0.,
    };

    vec![
        PMU::new(
            "A".to_string(),
            1,
            [0; 16],
            None,
            None,
            None,
            SynSVCClass::M,
            0,
            0,
            SynNominalFreq::F60Hz,
            120,
            calculation,
            4712,
        ),
        PMU::new(
            "B".to_string(),
            2,
            [0; 16],
            None,
            None,
            None,
            SynSVCClass::M,
            0,
            0,
            SynNominalFreq::F60Hz,
            120,
            calculation,
            4713,
        ),
        PMU::new(
            "C".to_string(),
            3,
            [0; 16],
            None,
            None,
            None,
            SynSVCClass::M,
            0,
            0,
            SynNominalFreq::F60Hz,
            120,
            calculation,
            4714,
        ),
    ]
}
