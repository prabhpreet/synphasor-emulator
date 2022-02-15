//! # Synchrophasor Emulator
//! Synchrophasor Emulator is a multi-stream PMU synchrophasor emulator, meant for simulating phasors of multiple PMUs.
//! The phasors can be simulated by specifying calculations in a closure in the `pmu_model` function as a function of `SystemTime`.
//! PMU stream parameters are also configurable in the same section. Applications of Synchrophasor Emulator could be to test
//! a wide-area control scheme.
//!

mod simulator;
use crate::simulator::*;
mod psmodel;
use crate::psmodel::*;
/// Entry point to Simulator module's asynchronous `start_model` function which starts the server and periodic synchrophasor calculations.
#[tokio::main]
async fn main() {
    let pmus = pmu_model();
    println!("Starting Simulator");
    crate::simulator::start_model(pmus).await;
}

/// Define and create PMU Streams, and phasor calculations.
/// Modify the calculation closure to simulate various conditions as a function of SystemTime.
/// # Example
/// ```
/// fn pmu_model() -> Vec<PMU> {
///    let calculation = |t: SystemTime| PMUData {
///        va_m: 120.,
///        va_a: 0.,
///        vb_m: 120.,
///        vb_a: -2. * PI / 3.,
///        vc_m: 120.,
///        vc_a: 2. * PI / 3.,
///        ia_m: 5.,
///        ia_a: 0.,
///        ib_m: 5.,
///        ib_a: -2. * PI / 3.,
///        ic_m: 5.,
///        ic_a: 2. * PI / 3.,
///        freq: 60.,
///        df_dt: 0.,
///    };
///
///    vec![
///        PMU::new(
///            "A".to_string(),
///            1,
///            [0; 16],
///            None,
///            None,
///            None,
///            SynSVCClass::M,
///            0,
///            0,
///            SynNominalFreq::F60Hz,
///            120,
///            calculation,
///            4712,
///        ),
///        PMU::new(
///            "B".to_string(),
///            2,
///            [0; 16],
///            None,
///            None,
///            None,
///            SynSVCClass::M,
///            0,
///            0,
///            SynNominalFreq::F60Hz,
///            120,
///            calculation,
///            4713,
///        ),
///        PMU::new(
///            "C".to_string(),
///            3,
///            [0; 16],
///            None,
///            None,
///            None,
///            SynSVCClass::M,
///            0,
///            0,
///            SynNominalFreq::F60Hz,
///            120,
///            calculation,
///            4714,
///        ),
///    ]
///}
/// ```
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
