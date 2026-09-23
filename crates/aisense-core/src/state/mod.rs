//! Detector de estado do agente (`docs/05`, "Calibração do detector de estado").

mod detector;

pub use detector::{Detection, StateConfidence, StateDetector, LOW_CONFIDENCE_SILENCE, TAIL_LINES};
