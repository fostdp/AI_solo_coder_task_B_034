class App {
    constructor() {
        this.currentElectrolyzerId = null;
        this.currentSensorId = null;
        this.electrolyzerCanvas = null;
        this.trendChart = null;
        this.efficiencyChart = null;
        this.modalTrendChart = null;
        this.electrolyzers = [];
        this.activeFilter = 'all';
        this.refreshInterval = null;
        
        this.init();
    }

    init() {
        this.setupEventListeners();
        this.startClock();
        this.loadInitialData();
        this.startAutoRefresh();
    }

    setupEventListeners() {
        document.getElementById('back-btn').addEventListener('click', () => this.showElectrolyzerList());
        
        document.getElementById('modal-close').addEventListener('click', () => this.closeSensorModal());
        document.getElementById('sensor-modal').addEventListener('click', (e) => {
            if (e.target.id === 'sensor-modal') this.closeSensorModal();
        });
        
        document.getElementById('optimization-modal-close').addEventListener('click', () => this.closeOptimizationModal());
        document.getElementById('optimization-modal').addEventListener('click', (e) => {
            if (e.target.id === 'optimization-modal') this.closeOptimizationModal();
        });
        
        document.querySelectorAll('.filter-buttons .btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                document.querySelectorAll('.filter-buttons .btn').forEach(b => b.classList.remove('active'));
                e.target.classList.add('active');
                this.activeFilter = e.target.dataset.filter;
                this.renderElectrolyzerList();
            });
        });
    }

    startClock() {
        const updateTime = () => {
            const now = new Date();
            document.getElementById('current-time').textContent = now.toLocaleString('zh-CN');
        };
        updateTime();
        setInterval(updateTime, 1000);
    }

    async loadInitialData() {
        try {
            await Promise.all([
                this.loadSystemSummary(),
                this.loadElectrolyzerList(),
                this.loadActiveAlerts(),
                this.loadOptimizationSuggestions()
            ]);
        } catch (error) {
            console.error('Failed to load initial data:', error);
            this.showNotification('数据加载失败，将使用模拟数据', 'warning');
            this.loadMockData();
        }
    }

    startAutoRefresh() {
        this.refreshInterval = setInterval(() => {
            this.refreshData();
        }, 5000);
    }

    async refreshData() {
        try {
            await Promise.all([
                this.loadSystemSummary(),
                this.loadElectrolyzerList(),
                this.loadActiveAlerts(),
                this.loadOptimizationSuggestions()
            ]);
            
            if (this.currentElectrolyzerId) {
                await this.loadElectrolyzerDetail(this.currentElectrolyzerId);
            }
        } catch (error) {
            console.error('Refresh failed:', error);
        }
    }

    async loadSystemSummary() {
        try {
            const summary = await api.getSystemSummary();
            this.updateSystemSummary(summary);
        } catch (error) {
            console.error('Failed to load system summary:', error);
            this.updateSystemSummary(this.getMockSystemSummary());
        }
    }

    updateSystemSummary(summary) {
        document.getElementById('total-hydrogen').textContent = summary.total_hydrogen?.toFixed(2) || '0.00';
        document.getElementById('avg-efficiency').textContent = summary.avg_efficiency?.toFixed(1) || '0.0';
        document.getElementById('total-power').textContent = summary.total_power?.toFixed(2) || '0.00';
        document.getElementById('active-electrolyzers').textContent = summary.active_electrolyzers || 0;
        
        const efficiencyTrend = document.getElementById('efficiency-trend');
        if (summary.avg_efficiency >= 78) {
            efficiencyTrend.textContent = '优秀';
            efficiencyTrend.className = 'metric-trend up';
        } else if (summary.avg_efficiency >= 75) {
            efficiencyTrend.textContent = '良好';
            efficiencyTrend.className = 'metric-trend';
        } else {
            efficiencyTrend.textContent = '需优化';
            efficiencyTrend.className = 'metric-trend down';
        }
    }

    async loadElectrolyzerList() {
        try {
            this.electrolyzers = await api.getElectrolyzerList();
            this.renderElectrolyzerList();
        } catch (error) {
            console.error('Failed to load electrolyzer list:', error);
            this.electrolyzers = this.getMockElectrolyzerList();
            this.renderElectrolyzerList();
        }
    }

    renderElectrolyzerList() {
        const grid = document.getElementById('electrolyzer-grid');
        grid.innerHTML = '';
        
        let filtered = this.electrolyzers;
        if (this.activeFilter !== 'all') {
            filtered = this.electrolyzers.filter(e => e.status === this.activeFilter);
        }
        
        filtered.forEach(electrolyzer => {
            const card = this.createElectrolyzerCard(electrolyzer);
            grid.appendChild(card);
        });
    }

    createElectrolyzerCard(electrolyzer) {
        const card = document.createElement('div');
        card.className = `electrolyzer-card status-${electrolyzer.status}`;
        
        if (electrolyzer.has_alert) {
            card.classList.add('has-alert');
        }
        
        const statusText = {
            'optimal': '最优',
            'normal': '正常',
            'warning': '警告'
        };
        
        card.innerHTML = `
            <div class="electrolyzer-id">#${electrolyzer.id}</div>
            <span class="electrolyzer-status">${statusText[electrolyzer.status] || '未知'}</span>
            <div class="electrolyzer-metrics">
                <div><span>效率</span><span>${electrolyzer.efficiency?.toFixed(1) || '--'}%</span></div>
                <div><span>电流密度</span><span>${electrolyzer.current_density?.toFixed(2) || '--'} A/cm²</span></div>
                <div><span>氢气产量</span><span>${electrolyzer.hydrogen_flow?.toFixed(3) || '--'} m³/h</span></div>
                <div><span>水温</span><span>${electrolyzer.water_temp?.toFixed(1) || '--'}°C</span></div>
            </div>
        `;
        
        card.addEventListener('click', () => this.showElectrolyzerDetail(electrolyzer.id));
        
        return card;
    }

    showElectrolyzerList() {
        this.currentElectrolyzerId = null;
        document.getElementById('electrolyzers-section').style.display = 'block';
        document.getElementById('detail-section').style.display = 'none';
    }

    async showElectrolyzerDetail(id) {
        this.currentElectrolyzerId = id;
        document.getElementById('electrolyzers-section').style.display = 'none';
        document.getElementById('detail-section').style.display = 'block';
        document.getElementById('detail-title').textContent = `电解槽 #${id} 详细信息`;
        
        if (!this.electrolyzerCanvas) {
            this.electrolyzerCanvas = new ElectrolyzerCanvas('electrolyzer-canvas');
            this.electrolyzerCanvas.onSensorClick = (sensor) => this.showSensorDetail(sensor);
            this.electrolyzerCanvas.onSensorHover = (sensor) => this.updateSensorInfoPanel(sensor);
        }
        
        if (!this.trendChart) {
            this.trendChart = new LineChart('trend-chart', {
                title: '参数趋势',
                xLabel: '时间',
                yLabel: '数值'
            });
        }
        
        if (!this.efficiencyChart) {
            this.efficiencyChart = new EfficiencyChart('efficiency-chart');
        }
        
        await this.loadElectrolyzerDetail(id);
    }

    async loadElectrolyzerDetail(id) {
        try {
            const detail = await api.getElectrolyzerDetail(id);
            this.updateElectrolyzerDetail(detail);
            
            const curves = await api.getEfficiencyCurves(id);
            this.updateEfficiencyChart(curves);
        } catch (error) {
            console.error('Failed to load electrolyzer detail:', error);
            const mockDetail = this.getMockElectrolyzerDetail(id);
            this.updateElectrolyzerDetail(mockDetail);
            this.updateEfficiencyChart(this.getMockEfficiencyCurves());
        }
    }

    updateElectrolyzerDetail(detail) {
        this.electrolyzerCanvas.setData(detail);
        
        document.getElementById('status-current').textContent = detail.current_density?.toFixed(2) || '--';
        document.getElementById('status-temp').textContent = detail.water_temp?.toFixed(1) || '--';
        document.getElementById('status-efficiency').textContent = detail.efficiency?.toFixed(1) || '--';
        document.getElementById('status-purity').textContent = detail.hydrogen_purity?.toFixed(2) || '--';
        document.getElementById('status-conductivity').textContent = detail.membrane_conductivity?.toFixed(4) || '--';
        
        if (detail.recent_data && detail.recent_data.length > 0) {
            const chartData = detail.recent_data.map(d => ({
                x: new Date(d.timestamp).getTime(),
                y: d.value
            }));
            this.trendChart.setData(chartData);
        }
    }

    updateEfficiencyChart(curves) {
        const currentElectrolyzer = this.electrolyzers.find(e => e.id === this.currentElectrolyzerId);
        const currentPoint = currentElectrolyzer ? {
            currentDensity: currentElectrolyzer.current_density,
            efficiency: currentElectrolyzer.efficiency
        } : null;
        
        this.efficiencyChart.setData(
            curves.efficiency_curve,
            curves.polarization_curve,
            currentPoint
        );
    }

    updateSensorInfoPanel(sensor) {
        const panel = document.getElementById('sensor-details');
        
        if (!sensor) {
            panel.innerHTML = '<p class="placeholder">点击传感器查看详细信息</p>';
            return;
        }
        
        const typeNames = {
            'voltage': '电压',
            'current_density': '电流密度',
            'hydrogen_flow': '氢气流量',
            'oxygen_flow': '氧气流量',
            'water_temp': '水温',
            'membrane_conductivity': '膜电导率',
            'hydrogen_purity': '氢气纯度',
            'cell_voltage': '小室电压'
        };
        
        const locationNames = {
            'anode': '阳极侧',
            'cathode': '阴极侧',
            'membrane': '膜电极'
        };
        
        const units = {
            'voltage': 'V',
            'current_density': 'A/cm²',
            'hydrogen_flow': 'm³/h',
            'oxygen_flow': 'm³/h',
            'water_temp': '°C',
            'membrane_conductivity': 'S/cm',
            'hydrogen_purity': '%',
            'cell_voltage': 'V'
        };
        
        const deviationClass = Math.abs(sensor.deviation_percent) < 3 ? 'deviation-normal' :
                               Math.abs(sensor.deviation_percent) < 7 ? 'deviation-warning' : 'deviation-danger';
        
        panel.innerHTML = `
            <div class="sensor-detail-item">
                <span class="sensor-detail-label">传感器ID</span>
                <span class="sensor-detail-value">#${sensor.sensor_id}</span>
            </div>
            <div class="sensor-detail-item">
                <span class="sensor-detail-label">类型</span>
                <span class="sensor-detail-value">${typeNames[sensor.sensor_type] || sensor.sensor_type}</span>
            </div>
            <div class="sensor-detail-item">
                <span class="sensor-detail-label">位置</span>
                <span class="sensor-detail-value">${locationNames[sensor.location] || sensor.location}</span>
            </div>
            <div class="sensor-detail-item">
                <span class="sensor-detail-label">当前值</span>
                <span class="sensor-detail-value">${sensor.current_value.toFixed(4)} ${units[sensor.sensor_type] || ''}</span>
            </div>
            <div class="sensor-detail-item">
                <span class="sensor-detail-label">额定值</span>
                <span class="sensor-detail-value">${sensor.rated_value.toFixed(4)} ${units[sensor.sensor_type] || ''}</span>
            </div>
            <div class="sensor-detail-item">
                <span class="sensor-detail-label">偏差</span>
                <span class="sensor-detail-value">
                    <span class="deviation-indicator ${deviationClass}">
                        ${sensor.deviation_percent >= 0 ? '+' : ''}${sensor.deviation_percent.toFixed(2)}%
                    </span>
                </span>
            </div>
        `;
    }

    async showSensorDetail(sensor) {
        this.currentSensorId = sensor.sensor_id;
        
        const modal = document.getElementById('sensor-modal');
        const modalTitle = document.getElementById('modal-title');
        const modalBody = document.getElementById('modal-body');
        
        const typeNames = {
            'voltage': '电压',
            'current_density': '电流密度',
            'hydrogen_flow': '氢气流量',
            'oxygen_flow': '氧气流量',
            'water_temp': '水温',
            'membrane_conductivity': '膜电导率',
            'hydrogen_purity': '氢气纯度',
            'cell_voltage': '小室电压'
        };
        
        const units = {
            'voltage': 'V',
            'current_density': 'A/cm²',
            'hydrogen_flow': 'm³/h',
            'oxygen_flow': 'm³/h',
            'water_temp': '°C',
            'membrane_conductivity': 'S/cm',
            'hydrogen_purity': '%',
            'cell_voltage': 'V'
        };
        
        modalTitle.textContent = `${typeNames[sensor.sensor_type] || sensor.sensor_type} #${sensor.sensor_id} 详情`;
        
        modalBody.innerHTML = `
            <div class="modal-info">
                <div class="modal-info-item">
                    <div class="modal-info-label">当前值</div>
                    <div class="modal-info-value">${sensor.current_value.toFixed(4)} ${units[sensor.sensor_type] || ''}</div>
                </div>
                <div class="modal-info-item">
                    <div class="modal-info-label">额定值</div>
                    <div class="modal-info-value">${sensor.rated_value.toFixed(4)} ${units[sensor.sensor_type] || ''}</div>
                </div>
                <div class="modal-info-item">
                    <div class="modal-info-label">偏差</div>
                    <div class="modal-info-value" style="color: ${this.getDeviationColor(sensor.deviation_percent)}">
                        ${sensor.deviation_percent >= 0 ? '+' : ''}${sensor.deviation_percent.toFixed(2)}%
                    </div>
                </div>
                <div class="modal-info-item">
                    <div class="modal-info-label">状态</div>
                    <div class="modal-info-value" style="color: ${this.getDeviationColor(sensor.deviation_percent)}">
                        ${this.getStatusText(sensor.deviation_percent)}
                    </div>
                </div>
            </div>
            <div class="modal-chart-container">
                <h4 style="margin-bottom: 0.5rem; color: #fff;">近2小时趋势</h4>
                <canvas id="modal-trend-canvas"></canvas>
            </div>
        `;
        
        modal.classList.add('active');
        
        setTimeout(() => {
            this.modalTrendChart = new ModalChart('modal-trend-canvas');
            this.loadSensorTrend(sensor);
        }, 100);
    }

    async loadSensorTrend(sensor) {
        try {
            const trendData = await api.getSensorDetail(this.currentElectrolyzerId, sensor.sensor_id, 2);
            
            if (trendData.trend_data && trendData.trend_data.length > 0) {
                this.modalTrendChart.setData(trendData.trend_data, {
                    lineColor: this.getDeviationColor(sensor.deviation_percent),
                    fillColor: this.getDeviationColor(sensor.deviation_percent).replace(')', ', 0.1)').replace('rgb', 'rgba')
                });
            }
        } catch (error) {
            console.error('Failed to load sensor trend:', error);
            const mockTrend = this.generateMockTrendData(sensor);
            this.modalTrendChart.setData(mockTrend, {
                lineColor: this.getDeviationColor(sensor.deviation_percent),
                fillColor: this.getDeviationColor(sensor.deviation_percent) + '20'
            });
        }
    }

    closeSensorModal() {
        document.getElementById('sensor-modal').classList.remove('active');
        this.currentSensorId = null;
        this.modalTrendChart = null;
    }

    async loadActiveAlerts() {
        try {
            const alerts = await api.getActiveAlerts();
            this.renderAlerts(alerts);
        } catch (error) {
            console.error('Failed to load active alerts:', error);
            this.renderAlerts(this.getMockAlerts());
        }
    }

    renderAlerts(alerts) {
        const list = document.getElementById('alerts-list');
        const countBadge = document.getElementById('alert-count');
        
        countBadge.textContent = `${alerts.length} 条活跃告警`;
        
        if (alerts.length === 0) {
            list.innerHTML = '<p class="placeholder">暂无告警信息</p>';
            return;
        }
        
        list.innerHTML = '';
        
        const levelNames = {
            1: '一级告警',
            2: '二级告警',
            3: '三级告警'
        };
        
        const typeNames = {
            'voltage': '电压异常',
            'hydrogen_purity': '氢气纯度异常',
            'membrane_degradation': '膜老化告警'
        };
        
        alerts.forEach(alert => {
            const item = document.createElement('div');
            item.className = `alert-item level-${alert.level}`;
            
            const time = new Date(alert.timestamp).toLocaleString('zh-CN');
            
            item.innerHTML = `
                <span class="alert-level">${levelNames[alert.level] || '未知'}</span>
                <div class="alert-content">
                    <div class="alert-type">电解槽 #${alert.electrolyzer_id} - ${typeNames[alert.alert_type] || alert.alert_type}</div>
                    <div class="alert-message">${alert.message}</div>
                    <div class="alert-time">${time}</div>
                </div>
                <div class="alert-actions">
                    <button class="btn btn-small" onclick="app.acknowledgeAlert('${alert.id}')">确认</button>
                    <button class="btn btn-small" onclick="app.resolveAlert('${alert.id}')">消除</button>
                </div>
            `;
            
            list.appendChild(item);
        });
    }

    async acknowledgeAlert(alertId) {
        try {
            await api.acknowledgeAlert(alertId);
            this.showNotification('告警已确认', 'success');
            await this.loadActiveAlerts();
        } catch (error) {
            console.error('Failed to acknowledge alert:', error);
            this.showNotification('操作失败', 'error');
        }
    }

    async resolveAlert(alertId) {
        try {
            await api.resolveAlert(alertId);
            this.showNotification('告警已消除', 'success');
            await this.loadActiveAlerts();
        } catch (error) {
            console.error('Failed to resolve alert:', error);
            this.showNotification('操作失败', 'error');
        }
    }

    async loadOptimizationSuggestions() {
        try {
            const suggestions = await api.getOptimizationSuggestions();
            this.renderOptimizationSuggestions(suggestions);
        } catch (error) {
            console.error('Failed to load optimization suggestions:', error);
            this.renderOptimizationSuggestions(this.getMockOptimizations());
        }
    }

    renderOptimizationSuggestions(suggestions) {
        const list = document.getElementById('optimization-list');
        
        if (suggestions.length === 0) {
            list.innerHTML = '<p class="placeholder">暂无优化建议</p>';
            return;
        }
        
        list.innerHTML = '';
        
        suggestions.forEach(suggestion => {
            const item = document.createElement('div');
            item.className = 'optimization-item';
            
            const time = new Date(suggestion.timestamp).toLocaleString('zh-CN');
            
            item.innerHTML = `
                <div class="optimization-content">
                    <div class="optimization-title">电解槽 #${suggestion.electrolyzer_id} 能效优化方案</div>
                    <div class="optimization-desc">${suggestion.description}</div>
                    <div class="optimization-metrics">
                        <div class="optimization-metric">
                            <span class="optimization-metric-label">当前效率</span>
                            <span class="optimization-metric-value">${suggestion.current_efficiency?.toFixed(1) || '--'}%</span>
                        </div>
                        <div class="optimization-metric">
                            <span class="optimization-metric-label">预期效率</span>
                            <span class="optimization-metric-value" style="color: #4CAF50;">${suggestion.expected_efficiency?.toFixed(1) || '--'}%</span>
                        </div>
                        <div class="optimization-metric">
                            <span class="optimization-metric-label">节能潜力</span>
                            <span class="optimization-metric-value" style="color: #FF9800;">${suggestion.energy_saving_potential?.toFixed(2) || '--'} kWh/h</span>
                        </div>
                    </div>
                    <div class="optimization-time">${time}</div>
                </div>
                <div class="optimization-actions">
                    <button class="btn btn-small" onclick="app.showOptimizationDetail('${suggestion.id}')">查看详情</button>
                </div>
            `;
            
            list.appendChild(item);
        });
    }

    showOptimizationDetail(suggestionId) {
        const suggestions = this.getMockOptimizations();
        const suggestion = suggestions.find(s => s.id === suggestionId) || suggestions[0];
        
        if (!suggestion) return;
        
        const modal = document.getElementById('optimization-modal');
        const modalBody = document.getElementById('optimization-modal-body');
        
        modalBody.innerHTML = `
            <div class="optimization-suggestion-detail">
                <h4>电解槽 #${suggestion.electrolyzer_id} 优化方案</h4>
                <p style="color: #aaa; margin-bottom: 1rem;">${suggestion.description}</p>
                
                <div class="optimization-params">
                    <div class="optimization-param">
                        <span class="param-label">电流密度</span>
                        <span>
                            <span class="param-value">${suggestion.optimized_params?.current_density?.current?.toFixed(2) || '--'} A/cm²</span>
                            <span class="param-arrow">→</span>
                            <span class="param-value" style="color: #4CAF50;">${suggestion.optimized_params?.current_density?.optimized?.toFixed(2) || '--'} A/cm²</span>
                        </span>
                    </div>
                    <div class="optimization-param">
                        <span class="param-label">去离子水温度</span>
                        <span>
                            <span class="param-value">${suggestion.optimized_params?.water_temperature?.current?.toFixed(1) || '--'}°C</span>
                            <span class="param-arrow">→</span>
                            <span class="param-value" style="color: #4CAF50;">${suggestion.optimized_params?.water_temperature?.optimized?.toFixed(1) || '--'}°C</span>
                        </span>
                    </div>
                </div>
                
                <div class="expected-efficiency">
                    <div class="expected-efficiency-label">预期提升后效率</div>
                    <div class="expected-efficiency-value">${suggestion.expected_efficiency?.toFixed(1) || '--'}%</div>
                </div>
            </div>
            
            <div class="modal-info">
                <div class="modal-info-item">
                    <div class="modal-info-label">当前效率</div>
                    <div class="modal-info-value" style="color: #FF9800;">${suggestion.current_efficiency?.toFixed(1) || '--'}%</div>
                </div>
                <div class="modal-info-item">
                    <div class="modal-info-label">效率提升</div>
                    <div class="modal-info-value" style="color: #4CAF50;">+${((suggestion.expected_efficiency || 0) - (suggestion.current_efficiency || 0)).toFixed(1)}%</div>
                </div>
                <div class="modal-info-item">
                    <div class="modal-info-label">节能潜力</div>
                    <div class="modal-info-value" style="color: #FF9800;">${suggestion.energy_saving_potential?.toFixed(2) || '--'} kWh/h</div>
                </div>
                <div class="modal-info-item">
                    <div class="modal-info-label">投资回收期</div>
                    <div class="modal-info-value">${suggestion.payback_period || '--'} 天</div>
                </div>
            </div>
            
            <button class="apply-btn" onclick="app.applyOptimization('${suggestion.id}')">应用优化方案</button>
        `;
        
        modal.classList.add('active');
    }

    async applyOptimization(suggestionId) {
        try {
            await api.applyOptimizationSuggestion(suggestionId);
            this.showNotification('优化方案已应用', 'success');
            this.closeOptimizationModal();
            await this.loadOptimizationSuggestions();
        } catch (error) {
            console.error('Failed to apply optimization:', error);
            this.showNotification('应用失败', 'error');
        }
    }

    closeOptimizationModal() {
        document.getElementById('optimization-modal').classList.remove('active');
    }

    getDeviationColor(deviationPercent) {
        const absDeviation = Math.abs(deviationPercent);
        if (absDeviation < 3) return '#4CAF50';
        else if (absDeviation < 7) return '#FFC107';
        else return '#F44336';
    }

    getStatusText(deviationPercent) {
        const absDeviation = Math.abs(deviationPercent);
        if (absDeviation < 3) return '正常';
        else if (absDeviation < 7) return '警告';
        else return '异常';
    }

    showNotification(message, type = 'info') {
        const colors = {
            success: '#4CAF50',
            error: '#F44336',
            warning: '#FFC107',
            info: '#2196F3'
        };
        
        const notification = document.createElement('div');
        notification.style.cssText = `
            position: fixed;
            top: 20px;
            right: 20px;
            background: ${colors[type]};
            color: white;
            padding: 1rem 1.5rem;
            border-radius: 8px;
            z-index: 2000;
            animation: slideIn 0.3s ease;
            box-shadow: 0 4px 15px rgba(0,0,0,0.3);
        `;
        notification.textContent = message;
        
        document.body.appendChild(notification);
        
        setTimeout(() => {
            notification.style.animation = 'slideIn 0.3s ease reverse';
            setTimeout(() => notification.remove(), 300);
        }, 3000);
    }

    loadMockData() {
        this.updateSystemSummary(this.getMockSystemSummary());
        this.electrolyzers = this.getMockElectrolyzerList();
        this.renderElectrolyzerList();
        this.renderAlerts(this.getMockAlerts());
        this.renderOptimizationSuggestions(this.getMockOptimizations());
    }

    getMockSystemSummary() {
        return {
            total_hydrogen: 1256.78,
            avg_efficiency: 76.8,
            total_power: 56789.23,
            active_electrolyzers: 10
        };
    }

    getMockElectrolyzerList() {
        const statuses = ['optimal', 'normal', 'normal', 'normal', 'warning', 'normal', 'optimal', 'normal', 'normal', 'warning'];
        return Array.from({ length: 10 }, (_, i) => ({
            id: i + 1,
            status: statuses[i],
            efficiency: 74 + Math.random() * 6,
            current_density: 1.8 + Math.random() * 0.8,
            hydrogen_flow: 25 + Math.random() * 5,
            water_temp: 75 + Math.random() * 10,
            has_alert: i === 4 || i === 9
        }));
    }

    getMockElectrolyzerDetail(id) {
        const sensors = [];
        const locations = ['anode', 'cathode', 'membrane'];
        const types = ['voltage', 'current_density', 'hydrogen_flow', 'oxygen_flow', 'water_temp', 'membrane_conductivity', 'hydrogen_purity', 'cell_voltage'];
        
        for (let i = 0; i < 50; i++) {
            const location = locations[i % 3];
            const type = types[i % types.length];
            const ratedValue = this.getRatedValue(type);
            const currentValue = ratedValue * (0.95 + Math.random() * 0.1);
            const deviationPercent = ((currentValue - ratedValue) / ratedValue) * 100;
            
            sensors.push({
                sensor_id: i + 1,
                sensor_type: type,
                location: location,
                x: 0.1 + (i % 10) * 0.08,
                y: 0.2 + Math.floor(i / 10) * 0.12,
                current_value: currentValue,
                rated_value: ratedValue,
                deviation_percent: deviationPercent
            });
        }
        
        const recentData = [];
        const now = Date.now();
        for (let i = 0; i < 3600; i++) {
            recentData.push({
                timestamp: now - (3600 - i) * 2000,
                value: 1.85 + Math.sin(i / 100) * 0.1 + Math.random() * 0.05
            });
        }
        
        return {
            id: id,
            current_density: 2.0,
            water_temp: 78.5,
            efficiency: 76.2,
            hydrogen_purity: 99.95,
            membrane_conductivity: 0.085,
            sensors: sensors,
            recent_data: recentData
        };
    }

    getRatedValue(type) {
        const ratedValues = {
            'voltage': 1.9,
            'current_density': 2.0,
            'hydrogen_flow': 30,
            'oxygen_flow': 15,
            'water_temp': 80,
            'membrane_conductivity': 0.1,
            'hydrogen_purity': 99.97,
            'cell_voltage': 1.85
        };
        return ratedValues[type] || 1.0;
    }

    getMockEfficiencyCurves() {
        const efficiencyCurve = [];
        const polarizationCurve = [];
        
        for (let j = 0.5; j <= 4.0; j += 0.1) {
            const reversibleVoltage = 1.229;
            const activationLoss = 0.05 * Math.log(Math.max(j / 0.001, 1));
            const ohmicLoss = 0.15 * j;
            const concentrationLoss = 0.02 * (1 - Math.exp(-j / 0.5));
            const cellVoltage = reversibleVoltage + activationLoss + ohmicLoss + concentrationLoss;
            const efficiency = (1.481 / cellVoltage) * 100;
            
            efficiencyCurve.push([j, efficiency]);
            polarizationCurve.push([j, cellVoltage]);
        }
        
        return { efficiencyCurve, polarizationCurve };
    }

    getMockAlerts() {
        return [
            {
                id: 'alert-001',
                level: 1,
                electrolyzer_id: 5,
                alert_type: 'voltage',
                message: '单槽电压超过2.0V，已持续5分钟',
                timestamp: new Date(Date.now() - 300000).toISOString(),
                acknowledged: false,
                resolved: false
            },
            {
                id: 'alert-002',
                level: 3,
                electrolyzer_id: 10,
                alert_type: 'membrane_degradation',
                message: '膜电导率下降超过20%，建议检查膜电极',
                timestamp: new Date(Date.now() - 600000).toISOString(),
                acknowledged: false,
                resolved: false
            }
        ];
    }

    getMockOptimizations() {
        return [
            {
                id: 'opt-001',
                electrolyzer_id: 5,
                description: '检测到效率低于75%，通过遗传算法优化得到以下参数调整建议',
                current_efficiency: 73.5,
                expected_efficiency: 78.8,
                energy_saving_potential: 12.5,
                payback_period: 72,
                timestamp: new Date(Date.now() - 1800000).toISOString(),
                optimized_params: {
                    current_density: { current: 2.4, optimized: 2.1 },
                    water_temperature: { current: 72, optimized: 78 }
                }
            },
            {
                id: 'opt-002',
                electrolyzer_id: 10,
                description: '膜老化导致效率下降，建议优化运行参数减缓衰减',
                current_efficiency: 74.2,
                expected_efficiency: 77.5,
                energy_saving_potential: 8.3,
                payback_period: 96,
                timestamp: new Date(Date.now() - 3600000).toISOString(),
                optimized_params: {
                    current_density: { current: 2.2, optimized: 1.9 },
                    water_temperature: { current: 75, optimized: 80 }
                }
            }
        ];
    }

    generateMockTrendData(sensor) {
        const data = [];
        const now = Date.now();
        const baseValue = sensor.rated_value;
        
        for (let i = 0; i < 3600; i++) {
            data.push({
                timestamp: now - (3600 - i) * 2000,
                value: baseValue * (0.97 + Math.sin(i / 50) * 0.02 + Math.random() * 0.01)
            });
        }
        
        return data;
    }
}

const style = document.createElement('style');
style.textContent = `
    @keyframes slideIn {
        from { transform: translateX(100%); opacity: 0; }
        to { transform: translateX(0); opacity: 1; }
    }
`;
document.head.appendChild(style);

const app = new App();
