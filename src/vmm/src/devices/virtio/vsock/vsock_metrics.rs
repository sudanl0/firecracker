// Copyright 2023 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Defines the metrics system for vsock devices.
//!
//! # Metrics format
//! The metrics are flushed in JSON when requested by vmm::logger::metrics::METRICS.write().
//!
//! ## JSON example with metrics:
//! ```json
//!  "vsock": {
//!     "activate_fails": "SharedIncMetric",
//!     "cfg_fails": "SharedIncMetric",
//!     "no_avail_buffer": "SharedIncMetric",
//!     "event_fails": "SharedIncMetric",
//!     "execute_fails": "SharedIncMetric",
//!     ...
//!  }
//! }
//! ```
//! Each `vsock` field in the example above is a serializable `vsockDeviceMetrics` structure
//! collecting metrics such as `activate_fails`, `cfg_fails`, etc. for the vsock device.
//! `vsock_drv0` represent metrics for the endpoint "/drives/drv0",
//! `vsock_drv1` represent metrics for the endpoint "/drives/drv1", and
//! `vsock_drive_id` represent metrics for the endpoint "/drives/{drive_id}"
//! vsock device respectively and `vsock` is the aggregate of all the per device metrics.
//!
//! # Limitations
//! vsock device currently do not have `vmm::logger::metrics::StoreMetrics` so aggregate
//! doesn't consider them.
//!
//! # Design
//! The main design goals of this system are:
//! * To improve vsock device metrics by logging them at per device granularity.
//! * Continue to provide aggregate vsock metrics to maintain backward compatibility.
//! * Move vsockDeviceMetrics out of from logger and decouple it.
//! * Rely on `serde` to provide the actual serialization for writing the metrics.
//! * Since all metrics start at 0, we implement the `Default` trait via derive for all of them, to
//!   avoid having to initialize everything by hand.
//!
//! * Devices could be created in any order i.e. the first device created could either be drv0 or
//!   drv1 so if we use a vector for vsockDeviceMetrics and call 1st device as vsock0, then vsock0
//!   could sometimes point to drv0 and sometimes to drv1 which doesn't help with analysing the
//!   metrics. So, use Map instead of Vec to help understand which drive the metrics actually
//!   belongs to.
//!
//! The system implements 1 type of metrics:
//! * Shared Incremental Metrics (SharedIncMetrics) - dedicated for the metrics which need a counter
//! (i.e the number of times an API request failed). These metrics are reset upon flush.
//! We add vsockDeviceMetrics entries from vsock_METRICS into vsock device instead of
//! vsock device having individual separate vsockDeviceMetrics entries because vsock device is not
//! accessible from signal handlers to flush metrics and vsock_METRICS is.

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

// use crate::logger::{IncMetric, SharedIncMetric};
use crate::logger::SharedIncMetric;

/// Pool of vsock-related metrics per device behind a lock to
/// keep things thread safe. Since the lock is initialized here
/// it is safe to unwrap it without any check.
pub static VSOCK_METRICS: VsockDeviceMetrics = VsockDeviceMetrics::new();

/// This function facilitates aggregation and serialization of
/// per vsock device metrics.
pub fn flush_metrics<S: Serializer>(serializer: S) -> Result<S::Ok, S::Error> {
    let mut seq = serializer.serialize_map(Some(1))?;
    seq.serialize_entry("vsock", &VSOCK_METRICS)?;
    seq.end()
}

/// Vsock-related metrics.
#[derive(Debug, Default, Serialize)]
pub struct VsockDeviceMetrics {
    /// Number of times when activate failed on a vsock device.
    pub activate_fails: SharedIncMetric,
    /// Number of times when interacting with the space config of a vsock device failed.
    pub cfg_fails: SharedIncMetric,
    /// Number of times when handling RX queue events on a vsock device failed.
    pub rx_queue_event_fails: SharedIncMetric,
    /// Number of times when handling TX queue events on a vsock device failed.
    pub tx_queue_event_fails: SharedIncMetric,
    /// Number of times when handling event queue events on a vsock device failed.
    pub ev_queue_event_fails: SharedIncMetric,
    /// Number of times when handling muxer events on a vsock device failed.
    pub muxer_event_fails: SharedIncMetric,
    /// Number of times when handling connection events on a vsock device failed.
    pub conn_event_fails: SharedIncMetric,
    /// Number of events associated with the receiving queue.
    pub rx_queue_event_count: SharedIncMetric,
    /// Number of events associated with the transmitting queue.
    pub tx_queue_event_count: SharedIncMetric,
    /// Number of bytes received.
    pub rx_bytes_count: SharedIncMetric,
    /// Number of transmitted bytes.
    pub tx_bytes_count: SharedIncMetric,
    /// Number of packets received.
    pub rx_packets_count: SharedIncMetric,
    /// Number of transmitted packets.
    pub tx_packets_count: SharedIncMetric,
    /// Number of added connections.
    pub conns_added: SharedIncMetric,
    /// Number of killed connections.
    pub conns_killed: SharedIncMetric,
    /// Number of removed connections.
    pub conns_removed: SharedIncMetric,
    /// How many times the killq has been resynced.
    pub killq_resync: SharedIncMetric,
    /// How many flush fails have been seen.
    pub tx_flush_fails: SharedIncMetric,
    /// How many write fails have been seen.
    pub tx_write_fails: SharedIncMetric,
    /// Number of times read() has failed.
    pub rx_read_fails: SharedIncMetric,
}

impl VsockDeviceMetrics {
    pub const fn new() -> Self {
        Self {
            activate_fails: SharedIncMetric::new(),
            cfg_fails: SharedIncMetric::new(),
            rx_queue_event_fails: SharedIncMetric::new(),
            tx_queue_event_fails: SharedIncMetric::new(),
            ev_queue_event_fails: SharedIncMetric::new(),
            muxer_event_fails: SharedIncMetric::new(),
            conn_event_fails: SharedIncMetric::new(),
            rx_queue_event_count: SharedIncMetric::new(),
            tx_queue_event_count: SharedIncMetric::new(),
            rx_bytes_count: SharedIncMetric::new(),
            tx_bytes_count: SharedIncMetric::new(),
            rx_packets_count: SharedIncMetric::new(),
            tx_packets_count: SharedIncMetric::new(),
            conns_added: SharedIncMetric::new(),
            conns_killed: SharedIncMetric::new(),
            conns_removed: SharedIncMetric::new(),
            killq_resync: SharedIncMetric::new(),
            tx_flush_fails: SharedIncMetric::new(),
            tx_write_fails: SharedIncMetric::new(),
            rx_read_fails: SharedIncMetric::new(),
        }
    }
}
