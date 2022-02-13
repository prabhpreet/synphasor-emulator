//! Module that runs PMU models, and serves PMU streams. Orchestrated through Tokio.rs asynchronous and concurrent threads

use crate::psmodel::*;
use serde_synphasor::de::SynphasorDeserializer;
use serde_synphasor::frame::baseframe::*;
use serde_synphasor::frame::cfgframes::*;
use serde_synphasor::frame::commandframe::*;
use serde_synphasor::frame::dataframe::*;
use serde_synphasor::ser::SynphasorSerializer;
pub use serde_synphasor::synstream::*;
use serde_synphasor::SynError;
use std::borrow::Borrow;
use std::collections::HashMap;
pub use std::f32::consts::PI;
use std::net::SocketAddr;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
pub use std::time::SystemTime;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

/// Reference Cell + Mutex data type for shared memory access of a hashmap of MPSC Senders. Based on the stream IDCODE, these senders are added/removed based on accepted socket connections
type TxStore = Arc<Mutex<HashMap<(u16, SocketAddr), tokio::sync::mpsc::Sender<WriteTask>>>>;

/// Start running the PMU Servers, and conduct calculations as per data_rate
pub async fn start_model(pmus: Vec<PMU>) {
    let tx_store: TxStore = Arc::new(Mutex::new(HashMap::new()));

    // Check if data_rates are equal
    let data_rates: Vec<i16> = pmus.iter().map(|pmu| pmu.data_rate).collect();
    if data_rates.iter().min() != data_rates.iter().max() {
        panic!("Stream Data Rates are not equal");
    }

    let pmu_data: Vec<(u16, u16, i16, Arc<Vec<u8>>)> = pmus
        .iter()
        .map(|pmu| {
            (
                pmu.idcode,
                pmu.port,
                pmu.data_rate,
                Arc::new(pmu.get_cfg_frame()),
            )
        })
        .collect();

    let tx_store_publish = tx_store.clone();

    let feeder_publish_handle = tokio::spawn(async move {
        feeder_publish(tx_store_publish, pmus).await;
    });

    for (idcode, port, data_rate, cfg_frame) in pmu_data.into_iter() {
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

/// Listen for incoming connections and create new concurrent Tokio thread on accepting new connection
async fn listen(listener: TcpListener, tx_store: TxStore, idcode: u16, cfg_frame: Arc<Vec<u8>>) {
    loop {
        let tx_store_publish = tx_store.clone();
        let cfg_frame_clone = cfg_frame.clone();
        let (socket, _) = listener.accept().await.unwrap();
        let peer_addr = socket.peer_addr().unwrap();
        let (rd, wr) = io::split(socket);
        let (tx_channel, mut rx_channel) = mpsc::channel::<WriteTask>(120);

        tokio::spawn(async move {
            read_stream(rd, tx_store_publish, tx_channel, idcode, peer_addr).await;
        });

        tokio::spawn(async move {
            write_stream(wr, rx_channel, cfg_frame_clone).await;
        });
    }
}

/// Write messages sent to write_stream asynchronous threads
enum WriteTask {
    SendCfg3Frame,
    SendDataFrame(Vec<u8>),
}

/// Write to stream asynchronously, including data frames
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

/// Read to stream asynchronously- any commands from PMU clients
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

/// Publish phasor data periodically
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
