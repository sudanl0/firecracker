// Copyright 2023 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Defines the metrics system for balloon devices.
//!
//! # Metrics format
//! The metrics are flushed in JSON when requested by vmm::logger::metrics::METRICS.write().
//!
//! ## JSON example with metrics:
//! ```json
//!  "balloon": {
//!     "activate_fails": "SharedIncMetric",
//!     "inflate_count": "SharedIncMetric",
//!     "stats_updates_count": "SharedIncMetric",
//!     ...
//!  }
//! }
//! ```
//! Each `balloon` field in the example above is a serializable `BalloonDeviceMetrics` structure
//! collecting metrics such as `activate_fails`, `inflate_count` etc. for the balloon device.
//! Since balloon doesn't support multiple devices, there is no per device metrics and
//! `balloon` represents the aggregate balloon metrics.
//!
//! # Design
//! The main design goals of this system are:
//! * Have a consistent approach of keeping device related metrics in the individual devices
//!   modules.
//! * To decouple balloon device metrics from logger module by moving BalloonDeviceMetrics out of
//!   FirecrackerDeviceMetrics.
//! * Rely on `serde` to provide the actual serialization for writing the metrics.
//!
//! The system implements 1 type of metrics:
//! * Shared Incremental Metrics (SharedIncMetrics) - dedicated for the metrics which need a counter
//! (i.e the number of times an API request failed). These metrics are reset upon flush.

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use crate::logger::SharedIncMetric;

/// Stores aggregated balloon metrics
pub(super) static BALLOON_METRICS: BalloonDeviceMetrics = BalloonDeviceMetrics::new();

/// Called by METRICS.flush(), this function facilitates serialization of balloon device metrics.
pub fn flush_metrics<S: Serializer>(serializer: S) -> Result<S::Ok, S::Error> {
    let mut seq = serializer.serialize_map(Some(1))?;
    seq.serialize_entry("balloon", &BALLOON_METRICS)?;
    seq.end()
}

/// Balloon Device associated metrics.
#[derive(Debug, Serialize)]
pub(super) struct BalloonDeviceMetrics {
    /// Number of times when activate failed on a balloon device.
    pub activate_fails: SharedIncMetric,
    /// Number of balloon device inflations.
    pub inflate_count: SharedIncMetric,
    // Number of balloon statistics updates from the driver.
    pub stats_updates_count: SharedIncMetric,
    // Number of balloon statistics update failures.
    pub stats_update_fails: SharedIncMetric,
    /// Number of balloon device deflations.
    pub deflate_count: SharedIncMetric,
    /// Number of times when handling events on a balloon device failed.
    pub event_fails: SharedIncMetric,
}
impl BalloonDeviceMetrics {
    /// Const default construction.
    const fn new() -> Self {
        Self {
            activate_fails: SharedIncMetric::new(),
            inflate_count: SharedIncMetric::new(),
            stats_updates_count: SharedIncMetric::new(),
            stats_update_fails: SharedIncMetric::new(),
            deflate_count: SharedIncMetric::new(),
            event_fails: SharedIncMetric::new(),
        }
    }
}
