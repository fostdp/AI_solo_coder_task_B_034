# PEM电解槽监控与能效优化平台

[![Rust](https://img.shields.io/badge/Rust-1.75+-dea584?logo=rust)](https://www.rust-lang.org/)
[![ClickHouse](https://img.shields.io/badge/ClickHouse-24.8-FFCC01?logo=clickhouse)](https://clickhouse.com/)
[![Docker](https://img.shields.io/badge/Docker-Compose-2496ED?logo=docker)](https://www.docker.com/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## 目录

- [系统架构](#系统架构)
- [核心功能](#核心功能)
- [快速部署](#快速部署)
- [服务详解](#服务详解)
- [Profinet模拟器](#profinet模拟器)
- [监控指标](#监控指标)
- [API文档](#api文档)
- [配置说明](#配置说明)
- [故障注入测试](#故障注入测试)
- [常见问题](#常见问题)

---

## 系统架构

### 整体架构图

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              Frontend (Nginx)                           │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────────────┐  │
│  │ StackProfile    │  │ EfficiencyDash  │  │ Gzip/Brotli Compression│  │
│  │ (Canvas 剖面图)  │  │ (效率面板)      │  │ Cache-Control: 1y     │  │
│  └────────┬────────┘  └────────┬────────┘  └───────────┬────────────┘  │
└───────────┼────────────────────┼──────────────────────────┼───────────────┘
            │                    │                          │
            ▼                    ▼                          ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        Backend (Rust + Axum + Tokio)                     │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │                      Central Coordinator (main.rs)                │  │
│  │  tokio::select! { ... } - 4路channel数据流                        │  │
│  └──────┬───────────────┬───────────────┬───────────────┬────────────┘  │
│         │               │               │               │               │
│  ┌──────▼─────┐  ┌──────▼─────┐  ┌──────▼─────┐  ┌──────▼─────┐       │
│  │ profinet_  │  │ efficiency_ │  │ optimization │ │ alarm_     │       │
│  │ driver     │  │ analyzer   │  │ _engine      │ │ bridge     │       │
│  │ (UDP 34567)│  │ (极化曲线)  │  │ (遗传算法+调度)│ │ (OPC UA)   │       │
│  └──────┬─────┘  └────────────┘  └──────┬─────┘  └──────┬─────┘       │
│         │                                │               │               │
└─────────┼────────────────────────────────┼───────────────┼───────────────┘
          │                                │               │
          ▼                                ▼               ▼
┌──────────────────────────┐  ┌───────────────────────┐  ┌──────────────┐
│   ClickHouse (时序库)     │  │  Prometheus (指标)    │  │  DCS / OPC UA │
│  - MergeTree + TTL       │  │  - port 9000/metrics  │  │  (告警推送)   │
│  - 三级存储: hot/warm/cold│  │  - Grafana集成        │  └──────────────┘
│  - 物化视图预聚合         │  └───────────────────────┘
└──────────────────────────┘
```

### 模块间Channel数据流

```
profinet_driver (UDP 34567)
    ↓ (SensorDataBatch)
main.rs: select! loop
    ↓ batch
efficiency_analyzer::analyze_batch()
    ↓ (EfficiencyResult)
main.rs
    ├─→ 更新缓存: latest_status, latest_sensors
    ├─→ DB: electrolyzer_status, efficiency_history
    ├─→ alarm_bridge::process_sensor_data()
    │   ↓ (Alert)
    │   main.rs
    │   ├─→ DB: alerts
    │   └─→ alarm_bridge::process_alert() → OPC UA → DCS
    └─→ efficiency_result.needs_optimization?
        ↓ 是
        optimization_engine::submit_optimization()
            ↓ (OptimizationSuggestion)
        main.rs
            ├─→ 更新缓存: latest_optimizations
            └─→ DB: optimization_suggestions
```

---

## 核心功能

| 功能模块 | 说明 |
|---------|------|
| **Profinet协议解析** | UDP端口34567接收，CRC-32校验，数据范围校验，每台电解槽50传感器×10台 |
| **能效评估** | 极化曲线拟合（Butler-Volmer方程），活化/浓差/欧姆损失计算，法拉第效率评估 |
| **智能优化** | 遗传算法（轮盘赌选择+精英保留），全局任务队列，Semaphore限制3并发 |
| **三级告警** | 高电压(5min)→L1，低纯度(3min)→L2，膜老化→L3，OPC UA推送至DCS |
| **可视化** | Canvas剖面图（7层MEA结构），传感器颜色编码（绿<3%/黄3-7%/红>7%） |
| **可观测性** | Tracing结构化日志，Prometheus指标（端口9000），ClickHouse三级存储 |

---

## 快速部署

### 前置要求

- Docker ≥ 24.0
- Docker Compose ≥ 2.20
- 至少 4核CPU / 8GB内存 / 20GB磁盘

### 一键部署

```bash
# 1. 克隆项目
git clone <repository>
cd AI_solo_coder_task_A_034

# 2. 启动所有服务（首次构建约10-15分钟）
docker-compose up -d --build

# 3. 检查服务状态
docker-compose ps
docker-compose logs -f backend
```

### 验证部署

```bash
# 检查ClickHouse
curl http://localhost:8123/ping
# 输出: Ok.

# 检查后端API
curl http://localhost:8080/api/electrolyzers
# 返回电解槽列表JSON

# 检查Prometheus指标
curl http://localhost:9000/metrics
# 返回Prometheus格式指标

# 检查模拟器API
curl http://localhost:8081/status
# 返回当前故障注入状态

# 访问前端
# 浏览器打开: http://localhost/
```

### 服务端口

| 服务 | 端口 | 说明 |
|------|------|------|
| 前端 | 80 | Nginx + Gzip |
| 后端API | 8080 | Axum REST API |
| Profinet | 34567/UDP | 模拟器数据接收 |
| Metrics | 9000 | Prometheus指标 |
| ClickHouse | 8123/9000 | HTTP/TCP接口 |
| 模拟器API | 8081 | 故障注入控制 |

### 停止服务

```bash
# 停止并保留数据
docker-compose down

# 停止并清理数据（谨慎使用）
docker-compose down -v
```

---

## 服务详解

### 1. ClickHouse时序数据库

#### 三级存储策略

| 层级 | 路径 | TTL | 用途 |
|------|------|-----|------|
| **Hot** | `/var/lib/clickhouse/hot` | 0-7天 | 最新数据，SSD，高IOPS |
| **Warm** | `/var/lib/clickhouse/warm` | 7天-1个月 | 近期历史，HDD |
| **Cold** | `/var/lib/clickhouse/cold` | 1-6个月 | 归档数据，大容量 |

#### 表结构设计

| 表名 | 分区 | TTL | 索引粒度 |
|------|------|-----|----------|
| sensor_data | 按天 | 1年 | 8192 |
| electrolyzer_status | 按天 | 1年 | 4096 |
| efficiency_history | 按天 | 1年 | 4096 |
| alerts | 按月 | 2年 | 8192 |
| optimization_suggestions | 按月 | 2年 | 4096 |

#### 物化视图

- `sensor_data_hourly_mv` - 小时级传感器数据聚合
- `electrolyzer_status_hourly_mv` - 小时级状态聚合

### 2. Rust后端

#### 可观测性指标（端口9000）

```
# Profinet
profinet_packets_received_total
profinet_packets_dropped_total{reason="crc_error|invalid_magic|out_of_range"}
profinet_crc_errors_total

# 运行指标
efficiency_percent{electrolyzer_id="1..10"}
hydrogen_production_m3h{electrolyzer_id="1..10"}
power_consumption_kw{electrolyzer_id="1..10"}
current_density_a_cm2{electrolyzer_id="1..10"}
water_temp_c{electrolyzer_id="1..10"}
cell_voltage_v{electrolyzer_id="1..10"}
membrane_conductivity_s_cm{electrolyzer_id="1..10"}
hydrogen_purity_percent{electrolyzer_id="1..10"}

# 告警
alerts_generated_total{level="level1|level2|level3"}
alerts_pushed_opcua_total{status="success|failed"}
opcua_connection_status

# 优化
optimization_tasks_submitted_total
optimization_tasks_completed_total{status="success|failed"}
optimization_queue_depth

# 系统
active_electrolyzers
db_writes_total{status="success|failed"}
sensor_data_points_total
```

#### 性能配置

- Tokio多线程运行时，自动CPU亲和
- ClickHouse批量写入（默认每批次500点）
- Semaphore限制3个并发遗传算法计算
- Channel容量：Profinet 1000、优化队列100

### 3. 前端

#### 组件拆分

| 组件 | 文件 | 功能 |
|------|------|------|
| **StackProfile** | [stack_profile.js](frontend/js/stack_profile.js) | 7层MEA剖面图，传感器标记点，点击/悬停交互 |
| **EfficiencyChart** | [efficiency_dashboard.js](frontend/js/efficiency_dashboard.js) | 效率曲线、极化曲线、双轴图 |
| **ModalChart** | [efficiency_dashboard.js](frontend/js/efficiency_dashboard.js) | 传感器趋势弹窗 |
| **LineChart** | [efficiency_dashboard.js](frontend/js/efficiency_dashboard.js) | 通用折线图组件 |

#### Gzip压缩配置

```nginx
gzip on;
gzip_comp_level 6;
gzip_min_length 256;
gzip_types text/plain text/css application/javascript application/json ...;
brotli on;
brotli_comp_level 6;
```

---

## Profinet模拟器

### 快速开始

```bash
# Docker方式（推荐）
docker-compose up simulator

# 或直接运行（需要Python 3.10+）
cd simulator
python3 profinet_simulator.py --initial-faults
```

### 命令行参数

```bash
python3 profinet_simulator.py \
  --host 127.0.0.1 \           # 目标主机
  --port 34567 \              # 目标端口
  --electrolyzers 10 \        # 电解槽数量
  --sensors 50 \              # 每台传感器数
  --interval 2.0 \            # 发送间隔(秒)
  --api-port 8081 \           # API端口
  --jitter 0.1 \              # 发送抖动(0-1)
  --initial-faults            # 启动演示故障
```

### 故障注入API（端口8081）

#### 查看状态

```bash
curl http://localhost:8081/status
```

#### 注入效率下降

```bash
# 为电解槽1、2、3注入效率下降，电流密度降低15%
curl -X POST http://localhost:8081/inject/efficiency_drop \
  -H "Content-Type: application/json" \
  -d '{"electrolyzer_ids": [1, 2, 3], "factor": 0.85}'

# 现象：电压升高到2.0V，水温升高5°C，效率下降到约65%
# 触发：遗传算法优化自动启动，尝试恢复到78%以上
```

#### 注入膜老化

```bash
# 为电解槽5注入膜老化，电导率以0.01/s速度下降
curl -X POST http://localhost:8081/inject/membrane_degradation \
  -H "Content-Type: application/json" \
  -d '{"electrolyzer_ids": [5], "rate": 0.01}'

# 现象：膜电导率持续下降，从0.12 S/cm逐步下降
# 触发：当电导率下降超过20%时，触发三级膜老化告警
```

#### 注入高电压

```bash
# 为电解槽1注入高电压
curl -X POST http://localhost:8081/inject/high_voltage \
  -H "Content-Type: application/json" \
  -d '{"electrolyzer_ids": [1]}'

# 现象：电压稳定在2.05V
# 触发：持续5分钟后触发一级告警（红色）
```

#### 注入低纯度

```bash
# 为电解槽2注入低纯度
curl -X POST http://localhost:8081/inject/low_purity \
  -H "Content-Type: application/json" \
  -d '{"electrolyzer_ids": [2]}'

# 现象：氢气纯度下降到99.85%
# 触发：持续3分钟后触发二级告警（橙色）
```

#### 重置故障

```bash
# 重置指定电解槽的效率下降
curl -X POST http://localhost:8081/reset_efficiency_drop \
  -H "Content-Type: application/json" \
  -d '{"electrolyzer_ids": [1, 2]}'

# 重置指定电解槽的膜老化（恢复电导率到0.12）
curl -X POST http://localhost:8081/reset_membrane_degradation \
  -H "Content-Type: application/json" \
  -d '{"electrolyzer_ids": [5]}'

# 清除所有故障
curl -X POST http://localhost:8081/clear -d '{}'

# 清除指定电解槽的所有故障
curl -X POST http://localhost:8081/clear \
  -H "Content-Type: application/json" \
  -d '{"electrolyzer_ids": [1, 2, 5]}'
```

### 数据包结构

```
Profinet Data Frame (Big Endian)
┌─────────────┬──────────────┬──────────────┬──────────────┐
│ Magic       │ Timestamp    │ Electrolyzer │ Sensor Count │
│ (4 bytes)   │ (4 bytes)    │ ID (1 byte)  │ (2 bytes)    │
│ 0x50524F4E  │ (ms)         │ 1-10         │ 50           │
├─────────────┴──────────────┴──────────────┴──────────────┤
│                     Sensor Data x 50                     │
├──────────┬──────────┬──────────┬────────────┬───────────┤
│ SensorID │ Type     │ Location │ Value      │ Rated     │
│ (2 bytes)│ (1 byte) │ (1 byte) │ (8 bytes)  │ (8 bytes) │
│ 1-50     │ 1-8      │ 1-3      │ f64        │ f64       │
├──────────┴──────────┴──────────┴────────────┴───────────┤
│                        CRC-32                           │
│                      (4 bytes)                          │
└─────────────────────────────────────────────────────────┘
Total: 12 + 50×20 + 4 = 1016 bytes/packet
Data Rate: 10槽 × 1016B / 2s = ~5 KB/s = ~18 MB/h
```

---

## API文档

### 电解槽相关

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/electrolyzers` | 获取所有电解槽状态 |
| GET | `/api/electrolyzers/:id` | 获取单个电解槽详情 |
| GET | `/api/electrolyzers/:id/sensors` | 获取电解槽传感器数据 |
| GET | `/api/electrolyzers/:id/efficiency` | 获取效率历史 |
| GET | `/api/electrolyzers/:id/optimizations` | 获取优化建议 |

### 告警相关

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/alerts` | 获取告警列表 |
| GET | `/api/alerts/:id` | 获取告警详情 |
| PUT | `/api/alerts/:id/acknowledge` | 确认告警 |
| PUT | `/api/alerts/:id/resolve` | 解决告警 |

### 系统相关

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/status` | 系统总览（H2产量、效率、电耗） |
| GET | `/api/opcua/status` | OPC UA连接状态 |
| GET | `/metrics` | Prometheus指标（端口9000） |

---

## 配置说明

### [config.toml](backend/config.toml) 核心配置

```toml
[server]
profinet_port = 34567
api_port = 8080

[database]
clickhouse_url = "http://clickhouse:8123"
clickhouse_database = "pem_electrolyzer"

[system]
electrolyzer_count = 10
sensors_per_electrolyzer = 50
active_area = 1.0

[optimization]
efficiency_threshold = 75.0      # 低于此值触发优化
target_efficiency = 78.0          # 优化目标
max_concurrent_optimizations = 3  # 最大并发优化数

[genetic_algorithm]
population_size = 100
mutation_rate = 0.1
crossover_rate = 0.8
max_generations = 100
elitism_count = 5

[alerts]
voltage_threshold = 2.0           # L1: >2.0V 持续5分钟
voltage_duration_seconds = 300
purity_threshold = 99.9           # L2: <99.9% 持续3分钟
purity_duration_seconds = 180
conductivity_degradation_threshold = 20.0  # L3: 下降>20%

[opcua]
server_url = "opc.tcp://dcs:4840"
heartbeat_interval_secs = 5
max_reconnect_delay_ms = 60000
```

### 环境变量覆盖

支持通过环境变量覆盖配置（前缀`APP_`，双下划线分隔）：

```bash
# 示例
export APP_DATABASE__CLICKHOUSE_URL=http://clickhouse:8123
export APP_OPTIMIZATION__EFFICIENCY_THRESHOLD=76.0
export APP_OPTIMIZATION__MAX_CONCURRENT_OPTIMIZATIONS=4
```

---

## 故障注入测试流程

### 标准测试场景

#### 场景1：效率下降 → 优化恢复

```bash
# 1. 注入效率下降
curl -X POST http://localhost:8081/inject/efficiency_drop \
  -d '{"electrolyzer_ids": [1], "factor": 0.80}'

# 2. 观察：约30秒后效率从~85%下降到~68%

# 3. 检查优化队列
curl http://localhost:9000/metrics | grep optimization_queue_depth

# 4. 观察：约2分钟后收到优化建议，效率恢复到78%+

# 5. 前端：电解槽1显示蓝色优化建议弹窗

# 6. 清理
curl -X POST http://localhost:8081/reset_efficiency_drop \
  -d '{"electrolyzer_ids": [1]}'
```

#### 场景2：膜老化 → 三级告警

```bash
# 1. 注入膜老化
curl -X POST http://localhost:8081/inject/membrane_degradation \
  -d '{"electrolyzer_ids": [5], "rate": 0.005}'

# 2. 观察：膜电导率从0.12逐步下降

# 3. 检查：当电导率下降到0.096以下（-20%），触发L3告警

# 4. 前端：电解槽5显示红色三级告警

# 5. 验证OPC UA推送
curl http://localhost:8080/api/opcua/status

# 6. 清理（恢复电导率）
curl -X POST http://localhost:8081/reset_membrane_degradation \
  -d '{"electrolyzer_ids": [5]}'
```

#### 场景3：10台电解槽同时优化

```bash
# 1. 为所有电解槽注入效率下降
curl -X POST http://localhost:8081/inject/efficiency_drop \
  -d '{"electrolyzer_ids": [1,2,3,4,5,6,7,8,9,10], "factor": 0.80}'

# 2. 观察队列深度（应为10）
curl http://localhost:9000/metrics | grep optimization_queue_depth

# 3. 观察：只有3个并发运行，其余排队

# 4. 检查活跃优化数
curl http://localhost:9000/metrics | grep optimization_tasks_completed_total

# 5. 清理所有
curl -X POST http://localhost:8081/clear -d '{}'
```

---

## 常见问题

### Q: 后端启动失败，无法连接ClickHouse？

A: 检查clickhouse服务是否健康：
```bash
docker-compose ps clickhouse
docker-compose logs clickhouse | tail -50
```

### Q: 前端无法连接API？

A: 检查Nginx反向代理配置：
```bash
docker-compose logs frontend
docker exec pem-frontend curl http://backend:8080/api/electrolyzers
```

### Q: 模拟器发送数据但前端无显示？

A: 检查UDP端口和数据包结构：
```bash
# 检查端口是否监听
docker exec pem-backend netstat -uln | grep 34567

# 检查数据包接收
docker-compose logs backend | grep -i "received\|batch"
```

### Q: 遗传算法优化不触发？

A: 检查效率阈值和实际效率：
```bash
curl http://localhost:9000/metrics | grep efficiency_percent
curl http://localhost:8080/api/electrolyzers/1 | jq '.average_efficiency'
```

### Q: 如何重置所有数据？

```bash
docker-compose down -v
rm -rf data/clickhouse/*
docker-compose up -d --build
```

---

## 目录结构

```
AI_solo_coder_task_A_034/
├── backend/                    # Rust后端
│   ├── src/
│   │   ├── main.rs            # 中央协调器
│   │   ├── lib.rs             # 库导出
│   │   ├── profinet_driver.rs # Profinet协议解析
│   │   ├── efficiency_analyzer.rs # 效率评估
│   │   ├── optimization_engine.rs  # 遗传算法+调度
│   │   ├── alarm_bridge.rs    # OPC UA+告警
│   │   ├── metrics.rs         # Prometheus指标
│   │   ├── config.rs          # TOML配置加载
│   │   ├── api.rs             # REST API
│   │   ├── db.rs              # ClickHouse操作
│   │   └── models.rs          # 数据模型
│   ├── Dockerfile             # 4阶段构建
│   ├── Cargo.toml
│   └── config.toml
├── frontend/                   # 前端
│   ├── js/
│   │   ├── stack_profile.js   # 剖面图组件
│   │   ├── efficiency_dashboard.js # 效率面板
│   │   ├── main.js            # 主入口
│   │   └── api.js             # API封装
│   ├── css/
│   ├── index.html
│   ├── nginx.conf             # Gzip+Brotli配置
│   └── Dockerfile
├── clickhouse/                 # ClickHouse配置
│   ├── config.xml             # 存储策略
│   └── init.sql               # Schema+TTL
├── simulator/                  # Profinet模拟器
│   ├── profinet_simulator.py  # 主程序
│   └── Dockerfile
├── data/                       # 本地数据卷
│   └── clickhouse/
│       ├── hot/
│       ├── warm/
│       ├── cold/
│       └── logs/
├── docker-compose.yml
└── README.md
```

---

## License

MIT
