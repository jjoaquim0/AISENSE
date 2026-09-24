//! Detector de estado do agente (`docs/05`, "Calibração do detector de estado").

mod detector;

pub use detector::{
    calibrate, Calibration, Detection, PatternCheck, StateConfidence, StateDetector,
    LOW_CONFIDENCE_SILENCE, TAIL_LINES,
};
