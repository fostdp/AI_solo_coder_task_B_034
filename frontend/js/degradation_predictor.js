class DegradationPredictorComponent {
    constructor(containerId) {
        this.container = document.getElementById(containerId);
        this.data = null;
        this.predictionChart = null;
        this.rulChart = null;
        this.render();
    }

    render() {
        if (!this.container) return;
        
        this.container.innerHTML = `
            <div class="degradation-container">
                <h3 class="component-title">性能退化预测</h3>
                
                <div class="degradation-status">
                    <div class="status-indicator" id="degradation-status-indicator">
                        <span class="status-dot"></span>
                        <span class="status-text" id="degradation-status-text">分析中...</span>
                    </div>
                    <div class="prediction-time" id="degradation-prediction-time"></div>
                </div>
                
                <div class="degradation-metrics">
                    <div class="metric-card">
                        <span class="metric-label">电压升高速率</span>
                        <span class="metric-value" id="degradation-voltage-rate">--</span>
                        <span class="metric-unit">V/1000h</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">效率衰减速率</span>
                        <span class="metric-value" id="degradation-efficiency-rate">--</span>
                        <span class="metric-unit">%/1000h</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">电阻升高速率</span>
                        <span class="metric-value" id="degradation-resistance-rate">--</span>
                        <span class="metric-unit">mΩ/1000h</span>
                    </div>
                    <div class="metric-card">
                        <span class="metric-label">性能指数</span>
                        <span class="metric-value" id="degradation-performance-index">--</span>
                        <span class="metric-unit">分</span>
                    </div>
                </div>
                
                <div class="degradation-rul">
                    <h4>剩余使用寿命 (RUL)</h4>
                    <div class="rul-display">
                        <div class="rul-main">
                            <span class="rul-value" id="degradation-rul-value">--</span>
                            <span class="rul-unit">天</span>
                        </div>
                        <div class="rul-bounds">
                            <span class="bound-label">置信区间 [</span>
                            <span class="bound-value" id="degradation-rul-lower">--</span>
                            <span class="bound-sep">,</span>
                            <span class="bound-value" id="degradation-rul-upper">--</span>
                            <span class="bound-label">] 天</span>
                        </div>
                        <div class="rul-confidence">
                            <span class="confidence-label">置信度</span>
                            <span class="confidence-value" id="degradation-confidence-level">95%</span>
                        </div>
                    </div>
                </div>
                
                <div class="degradation-features">
                    <h4>退化特征</h4>
                    <div class="feature-grid">
                        <div class="feature-item">
                            <span class="feature-label">累计运行时间</span>
                            <span class="feature-value" id="degradation-operating-hours">-- h</span>
                        </div>
                        <div class="feature-item">
                            <span class="feature-label">累计通量</span>
                            <span class="feature-value" id="degradation-total-charge">-- Ah</span>
                        </div>
                        <div class="feature-item">
                            <span class="feature-label">温度循环次数</span>
                            <span class="feature-value" id="degradation-temp-cycles">--</span>
                        </div>
                        <div class="feature-item">
                            <span class="feature-label">最大功率利用率</span>
                            <span class="feature-value" id="degradation-max-power">-- %</span>
                        </div>
                    </div>
                </div>
                
                <div class="degradation-prediction-chart">
                    <h4>电压退化预测 (高斯过程回归)</h4>
                    <canvas id="degradation-prediction-canvas"></canvas>
                </div>
                
                <div class="degradation-bayesian">
                    <h4>贝叶斯先验与迁移学习</h4>
                    <div class="bayesian-info">
                        <div class="bayesian-item">
                            <span class="bayesian-label">贝叶斯先验</span>
                            <span class="bayesian-value" id="degradation-bayesian-enabled">--</span>
                        </div>
                        <div class="bayesian-item">
                            <span class="bayesian-label">先验均值</span>
                            <span class="bayesian-value" id="degradation-prior-mean">-- V/1000h</span>
                        </div>
                        <div class="bayesian-item">
                            <span class="bayesian-label">先验强度</span>
                            <span class="bayesian-value" id="degradation-prior-strength">--</span>
                        </div>
                        <div class="bayesian-item">
                            <span class="bayesian-label">迁移学习</span>
                            <span class="bayesian-value" id="degradation-transfer-enabled">--</span>
                        </div>
                        <div class="bayesian-item">
                            <span class="bayesian-label">迁移源数量</span>
                            <span class="bayesian-value" id="degradation-transfer-sources">--</span>
                        </div>
                        <div class="bayesian-item">
                            <span class="bayesian-label">迁移权重</span>
                            <span class="bayesian-value" id="degradation-transfer-weight">--</span>
                        </div>
                    </div>
                </div>
                
                <div class="degradation-divergence">
                    <h4>预测稳定性</h4>
                    <div class="divergence-status" id="degradation-divergence-status">
                        <span class="status-indicator">
                            <span class="status-dot"></span>
                            <span class="status-text">预测稳定</span>
                        </span>
                    </div>
                    <div class="divergence-details">
                        <span class="detail-label">GP推理服务:</span>
                        <span class="detail-value" id="degradation-gp-service">--</span>
                    </div>
                </div>
                
                <div class="degradation-recommendations">
                    <h4>维护建议</h4>
                    <ul id="degradation-recommendations" class="recommendations-list">
                        <li>等待预测结果...</li>
                    </ul>
                </div>
            </div>
        `;
        
        this.predictionChart = new GPRegressionChart('degradation-prediction-canvas');
    }

    setData(data) {
        this.data = data;
        this.updateUI();
    }

    updateUI() {
        if (!this.data) return;
        
        const statusMap = {
            'normal': { class: 'status-normal', text: '健康' },
            'mild': { class: 'status-mild', text: '轻度退化' },
            'moderate': { class: 'status-moderate', text: '中度退化' },
            'severe': { class: 'status-severe', text: '严重退化' }
        };
        
        const status = statusMap[this.data.health_status] || statusMap['normal'];
        const indicator = document.getElementById('degradation-status-indicator');
        indicator.className = `status-indicator ${status.class}`;
        document.getElementById('degradation-status-text').textContent = status.text;
        
        if (this.data.timestamp) {
            const time = new Date(this.data.timestamp).toLocaleString('zh-CN');
            document.getElementById('degradation-prediction-time').textContent = `预测时间: ${time}`;
        }
        
        if (this.data.features) {
            const f = this.data.features;
            document.getElementById('degradation-voltage-rate').textContent = 
                f.voltage_increase_rate?.toFixed(4) || '--';
            document.getElementById('degradation-efficiency-rate').textContent = 
                f.efficiency_decay_rate?.toFixed(4) || '--';
            document.getElementById('degradation-resistance-rate').textContent = 
                (f.resistance_increase_rate * 1000)?.toFixed(2) || '--';
            document.getElementById('degradation-performance-index').textContent = 
                (f.performance_index * 100)?.toFixed(1) || '--';
            
            document.getElementById('degradation-operating-hours').textContent = 
                `${f.cumulative_operating_hours?.toFixed(0) || '--'} h`;
            document.getElementById('degradation-total-charge').textContent = 
                `${f.total_charge?.toFixed(1) || '--'} Ah`;
            document.getElementById('degradation-temp-cycles').textContent = 
                f.temperature_cycling_count || '--';
            document.getElementById('degradation-max-power').textContent = 
                `${f.max_power_pct?.toFixed(1) || '--'} %`;
        }
        
        document.getElementById('degradation-rul-value').textContent = 
            this.data.remaining_useful_life?.toFixed(1) || '--';
        document.getElementById('degradation-rul-lower').textContent = 
            this.data.rul_lower_bound?.toFixed(1) || '--';
        document.getElementById('degradation-rul-upper').textContent = 
            this.data.rul_upper_bound?.toFixed(1) || '--';
        document.getElementById('degradation-confidence-level').textContent = 
            `${this.data.confidence_level || 95}%`;
        
        if (this.data.current_degradation_rate !== undefined) {
            document.getElementById('degradation-voltage-rate').textContent = 
                this.data.current_degradation_rate.toFixed(4);
        }
        
        const bayesianEnabled = this.data.bayesian_prior_applied ? '已启用' : '未启用';
        document.getElementById('degradation-bayesian-enabled').textContent = bayesianEnabled;
        
        if (this.data.bayesian_prior) {
            document.getElementById('degradation-prior-mean').textContent = 
                `${this.data.bayesian_prior.mean_voltage_rate?.toFixed(4) || '--'} V/1000h`;
            document.getElementById('degradation-prior-strength').textContent = 
                this.data.bayesian_prior.strength?.toFixed(1) || '--';
        }
        
        const transferEnabled = this.data.transfer_learning_applied ? '已启用' : '未启用';
        document.getElementById('degradation-transfer-enabled').textContent = transferEnabled;
        
        if (this.data.transfer_info) {
            document.getElementById('degradation-transfer-sources').textContent = 
                this.data.transfer_info.source_count || '--';
            document.getElementById('degradation-transfer-weight').textContent = 
                this.data.transfer_info.weight?.toFixed(2) || '--';
        }
        
        document.getElementById('degradation-gp-service').textContent = 
            this.data.gp_service_used ? '独立服务运行中' : '本地计算';
        
        const divergenceEl = document.getElementById('degradation-divergence-status');
        if (this.data.prediction_divergent) {
            divergenceEl.innerHTML = `
                <span class="status-indicator status-severe">
                    <span class="status-dot"></span>
                    <span class="status-text">检测到发散，已应用约束</span>
                </span>
            `;
        } else {
            divergenceEl.innerHTML = `
                <span class="status-indicator status-normal">
                    <span class="status-dot"></span>
                    <span class="status-text">预测稳定</span>
                </span>
            `;
        }
        
        const recList = document.getElementById('degradation-recommendations');
        if (this.data.recommendations && this.data.recommendations.length > 0) {
            recList.innerHTML = this.data.recommendations.map(rec => 
                `<li>${rec}</li>`
            ).join('');
        } else {
            recList.innerHTML = '<li>设备运行正常，无需特殊维护</li>';
        }
        
        if (this.predictionChart && this.data.predictions) {
            this.predictionChart.setData({
                history: this.data.history_data,
                predictions: this.data.predictions,
                features: this.data.features
            });
        }
    }
}

class GPRegressionChart {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.data = { history: [], predictions: [], features: null };
        this.setupCanvas();
    }

    setupCanvas() {
        const rect = this.canvas.getBoundingClientRect();
        const dpr = window.devicePixelRatio || 1;
        
        this.canvas.width = rect.width * dpr;
        this.canvas.height = rect.height * dpr;
        this.ctx.scale(dpr, dpr);
        
        this.displayWidth = rect.width;
        this.displayHeight = rect.height;
        this.margin = { top: 20, right: 20, bottom: 40, left: 50 };
        this.chartWidth = this.displayWidth - this.margin.left - this.margin.right;
        this.chartHeight = this.displayHeight - this.margin.top - this.margin.bottom;
    }

    setData(data) {
        this.data = data || { history: [], predictions: [], features: null };
        this.render();
    }

    render() {
        this.ctx.clearRect(0, 0, this.displayWidth, this.displayHeight);
        
        this.ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
        this.ctx.fillRect(0, 0, this.displayWidth, this.displayHeight);
        
        if ((!this.data.history || this.data.history.length === 0) && 
            (!this.data.predictions || this.data.predictions.length === 0)) {
            this.ctx.fillStyle = '#666';
            this.ctx.font = '14px "Segoe UI", sans-serif';
            this.ctx.textAlign = 'center';
            this.ctx.fillText('暂无预测数据', this.displayWidth / 2, this.displayHeight / 2);
            return;
        }
        
        const allVoltages = [];
        const allTimes = [];
        
        if (this.data.history) {
            this.data.history.forEach(p => {
                allVoltages.push(p.cell_voltage);
                allTimes.push(p.timestamp);
            });
        }
        
        if (this.data.predictions) {
            this.data.predictions.forEach(p => {
                allVoltages.push(p.predicted_voltage);
                if (p.upper_bound) allVoltages.push(p.upper_bound);
                if (p.lower_bound) allVoltages.push(p.lower_bound);
                allTimes.push(p.timestamp);
            });
        }
        
        const yMin = Math.min(...allVoltages) * 0.99;
        const yMax = Math.max(...allVoltages) * 1.01;
        const tMin = Math.min(...allTimes);
        const tMax = Math.max(...allTimes);
        
        this.drawGrid(tMin, tMax, yMin, yMax);
        this.drawAxes(tMin, tMax, yMin, yMax);
        
        if (this.data.predictions && this.data.predictions.length > 0) {
            this.drawConfidenceBand(tMin, tMax, yMin, yMax);
        }
        
        if (this.data.history && this.data.history.length > 0) {
            this.drawHistoryData(tMin, tMax, yMin, yMax);
        }
        
        if (this.data.predictions && this.data.predictions.length > 0) {
            this.drawPredictionLine(tMin, tMax, yMin, yMax);
        }
        
        this.drawLegend();
    }

    drawGrid(tMin, tMax, yMin, yMax) {
        this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
        this.ctx.lineWidth = 1;
        
        for (let i = 0; i <= 5; i++) {
            const x = this.margin.left + (i / 5) * this.chartWidth;
            this.ctx.beginPath();
            this.ctx.moveTo(x, this.margin.top);
            this.ctx.lineTo(x, this.displayHeight - this.margin.bottom);
            this.ctx.stroke();
            
            const y = this.margin.top + (i / 5) * this.chartHeight;
            this.ctx.beginPath();
            this.ctx.moveTo(this.margin.left, y);
            this.ctx.lineTo(this.displayWidth - this.margin.right, y);
            this.ctx.stroke();
        }
    }

    drawAxes(tMin, tMax, yMin, yMax) {
        this.ctx.strokeStyle = '#888';
        this.ctx.lineWidth = 2;
        
        this.ctx.beginPath();
        this.ctx.moveTo(this.margin.left, this.displayHeight - this.margin.bottom);
        this.ctx.lineTo(this.displayWidth - this.margin.right, this.displayHeight - this.margin.bottom);
        this.ctx.stroke();
        
        this.ctx.beginPath();
        this.ctx.moveTo(this.margin.left, this.displayHeight - this.margin.bottom);
        this.ctx.lineTo(this.margin.left, this.margin.top);
        this.ctx.stroke();
        
        this.ctx.fillStyle = '#ccc';
        this.ctx.font = '11px "Segoe UI", sans-serif';
        this.ctx.textAlign = 'center';
        this.ctx.fillText('时间', this.displayWidth / 2, this.displayHeight - 10);
        
        this.ctx.save();
        this.ctx.translate(12, this.displayHeight / 2);
        this.ctx.rotate(-Math.PI / 2);
        this.ctx.fillText('电压 (V)', 0, 0);
        this.ctx.restore();
        
        this.ctx.textAlign = 'right';
        for (let i = 0; i <= 5; i++) {
            const y = this.displayHeight - this.margin.bottom - (i / 5) * this.chartHeight;
            const val = yMin + (i / 5) * (yMax - yMin);
            this.ctx.fillText(val.toFixed(3), this.margin.left - 5, y + 4);
        }
    }

    drawConfidenceBand(tMin, tMax, yMin, yMax) {
        const tRange = tMax - tMin;
        
        this.ctx.beginPath();
        
        this.data.predictions.forEach((p, i) => {
            const t = new Date(p.timestamp).getTime();
            const x = this.margin.left + ((t - tMin) / tRange) * this.chartWidth;
            const y = this.displayHeight - this.margin.bottom - 
                ((p.upper_bound - yMin) / (yMax - yMin)) * this.chartHeight;
            
            if (i === 0) {
                this.ctx.moveTo(x, y);
            } else {
                this.ctx.lineTo(x, y);
            }
        });
        
        for (let i = this.data.predictions.length - 1; i >= 0; i--) {
            const p = this.data.predictions[i];
            const t = new Date(p.timestamp).getTime();
            const x = this.margin.left + ((t - tMin) / tRange) * this.chartWidth;
            const y = this.displayHeight - this.margin.bottom - 
                ((p.lower_bound - yMin) / (yMax - yMin)) * this.chartHeight;
            this.ctx.lineTo(x, y);
        }
        
        this.ctx.closePath();
        this.ctx.fillStyle = 'rgba(0, 212, 255, 0.15)';
        this.ctx.fill();
    }

    drawHistoryData(tMin, tMax, yMin, yMax) {
        const tRange = tMax - tMin;
        
        this.ctx.fillStyle = '#4CAF50';
        this.data.history.forEach(p => {
            const t = new Date(p.timestamp).getTime();
            const x = this.margin.left + ((t - tMin) / tRange) * this.chartWidth;
            const y = this.displayHeight - this.margin.bottom - 
                ((p.cell_voltage - yMin) / (yMax - yMin)) * this.chartHeight;
            
            this.ctx.beginPath();
            this.ctx.arc(x, y, 3, 0, Math.PI * 2);
            this.ctx.fill();
        });
    }

    drawPredictionLine(tMin, tMax, yMin, yMax) {
        const tRange = tMax - tMin;
        
        this.ctx.strokeStyle = '#FF9800';
        this.ctx.lineWidth = 2;
        this.ctx.beginPath();
        
        this.data.predictions.forEach((p, i) => {
            const t = new Date(p.timestamp).getTime();
            const x = this.margin.left + ((t - tMin) / tRange) * this.chartWidth;
            const y = this.displayHeight - this.margin.bottom - 
                ((p.predicted_voltage - yMin) / (yMax - yMin)) * this.chartHeight;
            
            if (i === 0) {
                this.ctx.moveTo(x, y);
            } else {
                this.ctx.lineTo(x, y);
            }
        });
        this.ctx.stroke();
        
        this.ctx.strokeStyle = '#FF9800';
        this.ctx.lineWidth = 1;
        this.ctx.setLineDash([3, 3]);
        
        if (this.data.history && this.data.history.length > 0) {
            const lastHistory = this.data.history[this.data.history.length - 1];
            const firstPred = this.data.predictions[0];
            
            const t1 = new Date(lastHistory.timestamp).getTime();
            const x1 = this.margin.left + ((t1 - tMin) / tRange) * this.chartWidth;
            const y1 = this.displayHeight - this.margin.bottom - 
                ((lastHistory.cell_voltage - yMin) / (yMax - yMin)) * this.chartHeight;
            
            const t2 = new Date(firstPred.timestamp).getTime();
            const x2 = this.margin.left + ((t2 - tMin) / tRange) * this.chartWidth;
            const y2 = this.displayHeight - this.margin.bottom - 
                ((firstPred.predicted_voltage - yMin) / (yMax - yMin)) * this.chartHeight;
            
            this.ctx.beginPath();
            this.ctx.moveTo(x1, y1);
            this.ctx.lineTo(x2, y2);
            this.ctx.stroke();
        }
        
        this.ctx.setLineDash([]);
    }

    drawLegend() {
        this.ctx.font = '11px "Segoe UI", sans-serif';
        this.ctx.textAlign = 'left';
        
        let x = this.margin.left + 10;
        const y = this.margin.top + 15;
        
        this.ctx.fillStyle = '#4CAF50';
        this.ctx.beginPath();
        this.ctx.arc(x + 5, y - 2, 4, 0, Math.PI * 2);
        this.ctx.fill();
        this.ctx.fillStyle = '#ccc';
        this.ctx.fillText('历史数据', x + 15, y + 2);
        
        x += 100;
        this.ctx.strokeStyle = '#FF9800';
        this.ctx.lineWidth = 2;
        this.ctx.beginPath();
        this.ctx.moveTo(x, y - 2);
        this.ctx.lineTo(x + 20, y - 2);
        this.ctx.stroke();
        this.ctx.fillStyle = '#ccc';
        this.ctx.fillText('GP预测', x + 25, y + 2);
        
        x += 100;
        this.ctx.fillStyle = 'rgba(0, 212, 255, 0.3)';
        this.ctx.fillRect(x, y - 8, 20, 12);
        this.ctx.fillStyle = '#ccc';
        this.ctx.fillText('95%置信区间', x + 25, y + 2);
    }
}

const degradationStyle = document.createElement('style');
degradationStyle.textContent = `
    .degradation-container {
        padding: 1rem;
        background: rgba(0, 0, 0, 0.3);
        border-radius: 8px;
    }
    .component-title {
        color: #00d4ff;
        margin: 0 0 1rem 0;
        font-size: 1.2rem;
    }
    .degradation-status {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 1rem;
        padding: 0.75rem;
        background: rgba(0, 0, 0, 0.3);
        border-radius: 6px;
    }
    .status-indicator {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }
    .status-dot {
        width: 12px;
        height: 12px;
        border-radius: 50%;
        background: #666;
    }
    .status-normal .status-dot { background: #4CAF50; }
    .status-mild .status-dot { background: #FFC107; }
    .status-moderate .status-dot { background: #FF9800; }
    .status-severe .status-dot { background: #F44336; }
    .degradation-metrics {
        display: grid;
        grid-template-columns: repeat(4, 1fr);
        gap: 0.75rem;
        margin-bottom: 1rem;
    }
    .metric-card {
        background: rgba(0, 0, 0, 0.3);
        padding: 0.75rem;
        border-radius: 6px;
        text-align: center;
    }
    .metric-label {
        display: block;
        color: #888;
        font-size: 0.8rem;
        margin-bottom: 0.25rem;
    }
    .metric-value {
        display: block;
        color: #00d4ff;
        font-size: 1.2rem;
        font-weight: bold;
    }
    .metric-unit {
        display: block;
        color: #666;
        font-size: 0.75rem;
    }
    .rul-display {
        display: flex;
        justify-content: space-around;
        align-items: center;
        padding: 1rem;
        background: rgba(0, 0, 0, 0.3);
        border-radius: 6px;
    }
    .rul-main {
        text-align: center;
    }
    .rul-value {
        display: block;
        font-size: 2.5rem;
        font-weight: bold;
        color: #FF9800;
    }
    .rul-unit {
        color: #888;
        font-size: 1rem;
    }
    .rul-bounds {
        text-align: center;
    }
    .bound-label {
        color: #888;
    }
    .bound-value {
        color: #00d4ff;
        font-weight: bold;
    }
    .bound-sep {
        color: #666;
        margin: 0 0.25rem;
    }
    .rul-confidence {
        text-align: center;
    }
    .confidence-label {
        display: block;
        color: #888;
        margin-bottom: 0.25rem;
    }
    .confidence-value {
        color: #4CAF50;
        font-weight: bold;
    }
    .feature-grid {
        display: grid;
        grid-template-columns: repeat(4, 1fr);
        gap: 0.75rem;
    }
    .feature-item {
        background: rgba(0, 0, 0, 0.3);
        padding: 0.75rem;
        border-radius: 6px;
        text-align: center;
    }
    .feature-label {
        display: block;
        color: #888;
        font-size: 0.8rem;
        margin-bottom: 0.25rem;
    }
    .feature-value {
        display: block;
        color: #FF9800;
        font-weight: bold;
    }
    #degradation-prediction-canvas {
        width: 100%;
        height: 250px;
        border-radius: 6px;
        margin-top: 0.5rem;
    }
    .bayesian-info {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 0.75rem;
    }
    .bayesian-item {
        background: rgba(0, 0, 0, 0.3);
        padding: 0.75rem;
        border-radius: 6px;
        text-align: center;
    }
    .bayesian-label {
        display: block;
        color: #888;
        font-size: 0.8rem;
        margin-bottom: 0.25rem;
    }
    .bayesian-value {
        display: block;
        color: #9C27B0;
        font-weight: bold;
    }
    .divergence-status {
        margin-bottom: 0.5rem;
    }
    .divergence-details {
        color: #888;
        font-size: 0.9rem;
    }
    .detail-label {
        color: #666;
    }
    .detail-value {
        color: #00d4ff;
    }
    .recommendations-list {
        margin: 0;
        padding-left: 1.25rem;
        color: #ccc;
    }
    .recommendations-list li {
        margin-bottom: 0.5rem;
    }
    .degradation-rul, .degradation-features, .degradation-prediction-chart,
    .degradation-bayesian, .degradation-divergence, .degradation-recommendations {
        margin-bottom: 1rem;
    }
    .degradation-rul h4, .degradation-features h4, .degradation-prediction-chart h4,
    .degradation-bayesian h4, .degradation-divergence h4, .degradation-recommendations h4 {
        color: #ccc;
        margin-bottom: 0.5rem;
    }
`;
document.head.appendChild(degradationStyle);
