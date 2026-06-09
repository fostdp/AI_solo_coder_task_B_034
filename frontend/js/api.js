const API_BASE_URL = 'http://localhost:8080';

class ApiClient {
    constructor(baseUrl) {
        this.baseUrl = baseUrl;
    }

    async request(endpoint, options = {}) {
        const url = `${this.baseUrl}${endpoint}`;
        const defaultHeaders = {
            'Content-Type': 'application/json',
        };

        const config = {
            ...options,
            headers: {
                ...defaultHeaders,
                ...options.headers,
            },
        };

        try {
            const response = await fetch(url, config);
            const data = await response.json();
            
            if (!data.success) {
                throw new Error(data.error || 'Request failed');
            }
            
            return data.data;
        } catch (error) {
            console.error(`API Error [${endpoint}]:`, error);
            throw error;
        }
    }

    async getSystemSummary() {
        return this.request('/api/system/summary');
    }

    async getElectrolyzerList() {
        return this.request('/api/electrolyzers');
    }

    async getElectrolyzerDetail(id, hours = 2) {
        return this.request(`/api/electrolyzers/${id}?hours=${hours}`);
    }

    async getSensorDetail(electrolyzerId, sensorId, hours = 2) {
        return this.request(`/api/electrolyzers/${electrolyzerId}/sensors/${sensorId}?hours=${hours}`);
    }

    async getEfficiencyCurves(electrolyzerId) {
        return this.request(`/api/electrolyzers/${electrolyzerId}/curves`);
    }

    async getActiveAlerts() {
        return this.request('/api/alerts/active');
    }

    async acknowledgeAlert(alertId) {
        return this.request(`/api/alerts/acknowledge?alert_id=${alertId}`, {
            method: 'PUT',
        });
    }

    async resolveAlert(alertId) {
        return this.request(`/api/alerts/resolve?alert_id=${alertId}`, {
            method: 'PUT',
        });
    }

    async getOptimizationSuggestions() {
        return this.request('/api/optimizations');
    }

    async applyOptimizationSuggestion(suggestionId) {
        return this.request(`/api/optimizations/apply?suggestion_id=${suggestionId}`, {
            method: 'PUT',
        });
    }

    async healthCheck() {
        return this.request('/health');
    }
}

const api = new ApiClient(API_BASE_URL);
