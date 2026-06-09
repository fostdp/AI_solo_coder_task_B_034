CREATE DATABASE IF NOT EXISTS pem_electrolyzer;

USE pem_electrolyzer;

SET allow_experimental_analyzer = 1;

CREATE TABLE IF NOT EXISTS sensor_data (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    electrolyzer_id UInt8,
    sensor_id UInt16,
    sensor_type Enum8(
        'voltage' = 1,
        'current_density' = 2,
        'hydrogen_flow' = 3,
        'oxygen_flow' = 4,
        'water_temp' = 5,
        'membrane_conductivity' = 6,
        'hydrogen_purity' = 7,
        'cell_voltage' = 8
    ),
    location Enum8(
        'anode' = 1,
        'cathode' = 2,
        'membrane' = 3
    ),
    value Float64,
    rated_value Float64,
    INDEX idx_electrolyzer_sensor (electrolyzer_id, sensor_id) TYPE minmax GRANULARITY 1,
    INDEX idx_sensor_type (sensor_type) TYPE set(1000) GRANULARITY 1,
    INDEX idx_timestamp (timestamp) TYPE minmax GRANULARITY 1
) ENGINE = MergeTree()
PARTITION BY toYYYYMMDD(timestamp)
ORDER BY (electrolyzer_id, sensor_id, timestamp)
PRIMARY KEY (electrolyzer_id, sensor_id, timestamp)
TTL timestamp + INTERVAL 7 DAY TO VOLUME 'hot',
    timestamp + INTERVAL 1 MONTH TO VOLUME 'warm',
    timestamp + INTERVAL 6 MONTH TO VOLUME 'cold',
    timestamp + INTERVAL 1 YEAR DELETE
SETTINGS
    index_granularity = 8192,
    min_bytes_for_wide_part = '10Mi',
    max_parts_in_total = 10000,
    merge_with_ttl_timeout = 3600,
    storage_policy = 'tiered';

CREATE TABLE IF NOT EXISTS electrolyzer_status (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    electrolyzer_id UInt8,
    total_hydrogen_production Float64,
    average_efficiency Float64,
    total_power_consumption Float64,
    cell_voltage Array(Float64),
    current_density Float64,
    water_temp Float64,
    hydrogen_purity Float64,
    membrane_conductivity Float64,
    INDEX idx_electrolyzer (electrolyzer_id) TYPE minmax GRANULARITY 1,
    INDEX idx_efficiency (average_efficiency) TYPE minmax GRANULARITY 1
) ENGINE = MergeTree()
PARTITION BY toYYYYMMDD(timestamp)
ORDER BY (electrolyzer_id, timestamp)
PRIMARY KEY (electrolyzer_id, timestamp)
TTL timestamp + INTERVAL 1 MONTH TO VOLUME 'warm',
    timestamp + INTERVAL 6 MONTH TO VOLUME 'cold',
    timestamp + INTERVAL 1 YEAR DELETE
SETTINGS
    index_granularity = 4096,
    min_bytes_for_wide_part = '5Mi',
    max_parts_in_total = 5000,
    storage_policy = 'tiered';

CREATE TABLE IF NOT EXISTS alerts (
    id UUID DEFAULT generateUUIDv4(),
    timestamp DateTime64(3, 'Asia/Shanghai'),
    electrolyzer_id UInt8,
    alert_level Enum8(
        'level1' = 1,
        'level2' = 2,
        'level3' = 3
    ),
    alert_type String,
    message String,
    value Float64,
    threshold Float64,
    acknowledged Bool DEFAULT false,
    resolved Bool DEFAULT false,
    resolved_at Nullable(DateTime64(3, 'Asia/Shanghai')),
    INDEX idx_alert_level (alert_level) TYPE set(10) GRANULARITY 1,
    INDEX idx_electrolyzer (electrolyzer_id) TYPE minmax GRANULARITY 1,
    INDEX idx_resolved (resolved) TYPE set(2) GRANULARITY 1
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (timestamp, alert_level, electrolyzer_id)
PRIMARY KEY (timestamp, alert_level, electrolyzer_id)
TTL timestamp + INTERVAL 3 MONTH TO VOLUME 'warm',
    timestamp + INTERVAL 1 YEAR TO VOLUME 'cold',
    timestamp + INTERVAL 2 YEAR DELETE
SETTINGS
    index_granularity = 8192,
    max_parts_in_total = 3000,
    storage_policy = 'tiered';

CREATE TABLE IF NOT EXISTS optimization_suggestions (
    id UUID DEFAULT generateUUIDv4(),
    timestamp DateTime64(3, 'Asia/Shanghai'),
    electrolyzer_id UInt8,
    current_efficiency Float64,
    optimized_current_density Float64,
    optimized_water_temp Float64,
    expected_efficiency Float64,
    applied Bool DEFAULT false,
    applied_at Nullable(DateTime64(3, 'Asia/Shanghai')),
    INDEX idx_electrolyzer (electrolyzer_id) TYPE minmax GRANULARITY 1,
    INDEX idx_applied (applied) TYPE set(2) GRANULARITY 1
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (electrolyzer_id, timestamp)
PRIMARY KEY (electrolyzer_id, timestamp)
TTL timestamp + INTERVAL 3 MONTH TO VOLUME 'warm',
    timestamp + INTERVAL 1 YEAR TO VOLUME 'cold',
    timestamp + INTERVAL 2 YEAR DELETE
SETTINGS
    index_granularity = 4096,
    max_parts_in_total = 3000,
    storage_policy = 'tiered';

CREATE TABLE IF NOT EXISTS efficiency_history (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    electrolyzer_id UInt8,
    current_density Float64,
    cell_voltage Float64,
    efficiency Float64,
    water_temp Float64,
    INDEX idx_electrolyzer (electrolyzer_id) TYPE minmax GRANULARITY 1,
    INDEX idx_efficiency (efficiency) TYPE minmax GRANULARITY 1
) ENGINE = MergeTree()
PARTITION BY toYYYYMMDD(timestamp)
ORDER BY (electrolyzer_id, timestamp)
PRIMARY KEY (electrolyzer_id, timestamp)
TTL timestamp + INTERVAL 1 MONTH TO VOLUME 'warm',
    timestamp + INTERVAL 6 MONTH TO VOLUME 'cold',
    timestamp + INTERVAL 1 YEAR DELETE
SETTINGS
    index_granularity = 4096,
    min_bytes_for_wide_part = '5Mi',
    max_parts_in_total = 5000,
    storage_policy = 'tiered';

CREATE MATERIALIZED VIEW IF NOT EXISTS sensor_data_hourly_mv
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (electrolyzer_id, sensor_type, toStartOfHour(timestamp))
AS SELECT
    toStartOfHour(timestamp) AS timestamp,
    electrolyzer_id,
    sensor_type,
    avg(value) AS avg_value,
    min(value) AS min_value,
    max(value) AS max_value,
    count() AS sample_count
FROM sensor_data
GROUP BY electrolyzer_id, sensor_type, toStartOfHour(timestamp);

CREATE MATERIALIZED VIEW IF NOT EXISTS electrolyzer_status_hourly_mv
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (electrolyzer_id, toStartOfHour(timestamp))
AS SELECT
    toStartOfHour(timestamp) AS timestamp,
    electrolyzer_id,
    avg(average_efficiency) AS avg_efficiency,
    avg(total_hydrogen_production) AS avg_hydrogen_production,
    avg(total_power_consumption) AS avg_power_consumption,
    sum(total_hydrogen_production) AS total_hydrogen,
    sum(total_power_consumption) AS total_power,
    count() AS sample_count
FROM electrolyzer_status
GROUP BY electrolyzer_id, toStartOfHour(timestamp);

CREATE TABLE IF NOT EXISTS system_summary (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    total_hydrogen Float64,
    avg_efficiency Float64,
    total_power Float64,
    active_electrolyzers UInt8,
    INDEX idx_active (active_electrolyzers) TYPE minmax GRANULARITY 1
) ENGINE = MergeTree()
PARTITION BY toYYYYMMDD(timestamp)
ORDER BY timestamp
PRIMARY KEY timestamp
TTL timestamp + INTERVAL 1 MONTH TO VOLUME 'warm',
    timestamp + INTERVAL 6 MONTH TO VOLUME 'cold',
    timestamp + INTERVAL 1 YEAR DELETE
SETTINGS
    index_granularity = 4096,
    max_parts_in_total = 3000,
    storage_policy = 'tiered';
