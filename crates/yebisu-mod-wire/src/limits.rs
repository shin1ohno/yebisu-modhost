//! Appendix A8 ceilings; peers may only negotiate smaller values.
pub const PUBLISH_BYTES: usize = 1_048_576;
pub const PUBLISH_DECOMPRESSED_BYTES: usize = 4_194_304;
pub const NODES: usize = 20_000;
pub const DEPTH: usize = 256;
pub const VISITS: usize = 20_000;
pub const STORED_OPS: usize = 160_000;
pub const EXPANDED_OPS: usize = 160_000;
pub const STRING_BYTES: usize = 262_144;
pub const STRINGS_BYTES: usize = 1_048_576;
pub const ENTITIES: usize = 1_024;
pub const ENTITY_STRING_BYTES: usize = 65_536;
pub const ENTITY_STATE_BYTES: usize = 1_048_576;
pub const COPIES_HOST: usize = 16;
pub const COPIES_APP: usize = 32;
pub const CALLBACK_GENERATIONS: usize = 3;
pub const CALLBACKS: usize = 10_000;
pub const EVENT_QUEUE: usize = 64;
pub const CONTROL_BYTES: usize = 65_536;
pub const CONTROL_PENDING_BYTES: usize = 262_144;
pub const CONTROL_PER_SECOND: usize = 64;
pub const RECEIPTS: usize = 1_024;
pub const RECEIPTS_BYTES: usize = 1_048_576;
pub const ROWS: usize = 200;
pub const ROWS_BYTES: usize = 262_144;
pub const LIST_ITEMS: usize = 10_000;
pub const LISTS: usize = 32;
pub const ROW_REQUESTS_LIST: usize = 2;
pub const ROW_REQUESTS_COPY: usize = 8;
pub const ROW_REQUESTS_APP: usize = 32;
pub const ROW_CACHE_BYTES: usize = 8_388_608;
pub const IMAGE_BYTES: usize = 4_194_304;
pub const IMAGE_DIMENSION: usize = 4_096;
pub const SVG_BYTES: usize = 524_288;
pub const ASSET_CACHE_BYTES: usize = 16_777_216;
pub const ASSET_TRANSFERS_COPY: usize = 2;
pub const ASSET_TRANSFERS_HOST: usize = 4;
pub const ASSET_TRANSFERS_APP: usize = 8;
pub const ASSET_DECODED_BYTES: usize = 16_777_216;
pub const ASSETS_DECODED_BYTES: usize = 67_108_864;
pub const ASSETS_PIXELS: usize = 16_000_000;
pub const APP_BYTES: usize = 134_217_728;
pub const JS_HEAP_BYTES: usize = 67_108_864;
pub const WORKER_RSS_BYTES: usize = 134_217_728;
pub const WORKERS_RSS_BYTES: usize = 536_870_912;
pub const CHILDREN_COPY: usize = 2;
pub const CHILDREN_HOST: usize = 8;
pub const BACKGROUND_COPIES: usize = 4;
pub const JS_CALL_MS: usize = 50;
pub const VALIDATE_SLICE_MS: usize = 10;
pub const MATERIALIZE_SLICE_MS: usize = 4;
pub const CPU_COPY_MS_PER_SECOND: usize = 100;
pub const CPU_RUNNER_MS_PER_SECOND: usize = 500;
pub const CPU_IPAD_MS_PER_SECOND: usize = 250;
pub const HELLO_TIMEOUT_MS: usize = 5_000;
pub const PUBLISH_TIMEOUT_MS: usize = 5_000;
pub const OP_TIMEOUT_MS: usize = 5_000;
pub const WRITE_TIMEOUT_MS: usize = 5_000;
pub const CREDIT_BYTES: usize = 262_144;
pub const CHUNK_BYTES: usize = 16_384;
pub const LEASE_MS: usize = 30_000;
pub const LEASE_RENEW_MS: usize = 10_000;
pub const RESUME_EDIT_MS: usize = 2_000;
pub const RECEIPT_RETENTION_MS: usize = 60_000;
pub const EDIT_BATCH_MS: usize = 50;
pub const GRANT_MS: usize = 1_000;
pub const BACKGROUND_TTL_SECS: usize = 86_400;
pub const FRAME_HZ: usize = 30;
pub const STOP_LOG_BYTES: usize = 1_048_576;
pub const CHILD_TERM_MS: usize = 2_000;
pub const CHILD_REAP_MS: usize = 3_000;
pub const PRIORITY_DEBOUNCE_MS: usize = 2_000;
pub const RECONCILE_TIMEOUT_MS: usize = 5_000;
pub const CLOCK_SYNC_TIMEOUT_MS: usize = 600_000;
pub const JSON_DEPTH: usize = 64;
pub const OTP_DIGITS: usize = 32;
pub const CALENDAR_MONTHS: usize = 12;
pub const COORDINATE_ABS: f64 = 1_000_000.0;

// Runtime values the design fixes outside the A8 table. They are not negotiated;
// the runner and renderer read them from here so a G1-G5 revision changes one place.
/// A13 `notify`: at most one new toast per second.
pub const TOASTS_PER_SECOND: usize = 1;
/// A13 `notify`: at most three toasts shown at the same time.
pub const TOASTS_VISIBLE: usize = 3;
/// A14: the in-memory contact origin reaches the grant file at most this late.
pub const CONTACT_WRITE_MS: usize = 60_000;
/// A14: `wall_high_water` refresh period while no valid contact advances it.
pub const WALL_HIGH_WATER_MS: usize = 60_000;
/// A14: re-check period while waiting for the OS to report clock sync.
pub const CLOCK_SYNC_RECHECK_MS: usize = 60_000;
/// A14 (b): after an OS reboot, a wall clock this far behind `wall_high_water`
/// is not trusted and the grant is not restored.
pub const WALL_ROLLBACK_SECS: usize = 300;
