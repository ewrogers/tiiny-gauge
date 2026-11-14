pub(crate) enum SignalStrength {
    Poor,
    Good,
    Excellent,
}

impl SignalStrength {
    pub(crate) fn from_rssi(rssi: i32) -> Self {
        match rssi {
            ..=-76 => Self::Poor,
            -75..=-61 => Self::Good,
            _ => Self::Excellent,
        }
    }

    pub(crate) fn bars(&self) -> usize {
        match self {
            Self::Poor => 1,
            Self::Good => 2,
            Self::Excellent => 3,
        }
    }

    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Poor => "Poor",
            Self::Good => "Good",
            Self::Excellent => "Excellent",
        }
    }
}
