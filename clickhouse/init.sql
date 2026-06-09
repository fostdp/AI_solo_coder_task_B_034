CREATE DATABASE IF NOT EXISTS pem_electrolyzer;

USE pem_electrolyzer;

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
    rated_value Float64
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (electrolyzer_id, sensor_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
SETTINGS index_granularity = 8192;

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
    membrane_conductivity Float64
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (electrolyzer_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
SETTINGS index_granularity = 8192;

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
    resolved Bool DEFAULT false
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (timestamp, alert_level, electrolyzer_id)
TTL timestamp + INTERVAL 2 YEAR
SETTINGS index_granularity = 8192;

CREATE TABLE IF NOT EXISTS optimization_suggestions (
    id UUID DEFAULT generateUUIDv4(),
    timestamp DateTime64(3, 'Asia/Shanghai'),
    electrolyzer_id UInt8,
    current_efficiency Float64,
    optimized_current_density Float64,
    optimized_water_temp Float64,
    expected_efficiency Float64,
    applied Bool DEFAULT false
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (electrolyzer_id, timestamp)
TTL timestamp + INTERVAL 2 YEAR
SETTINGS index_granularity = 8192;

CREATE TABLE IF NOT EXISTS efficiency_history (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    electrolyzer_id UInt8,
    current_density Float64,
    cell_voltage Float64,
    efficiency Float64,
    water_temp Float64
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (electrolyzer_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
SETTINGS index_granularity = 8192;

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

CREATE TABLE IF NOT EXISTS system_summary (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    total_hydrogen Float64,
    avg_efficiency Float64,
    total_power Float64,
    active_electrolyzers UInt8
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY timestamp
TTL timestamp + INTERVAL 1 YEAR
SETTINGS index_granularity = 8192;
