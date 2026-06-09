use crate::models::*;
use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use rand::Rng;
use rand_distr::{Distribution, Normal};
use std::collections::HashSet;
use std::f64::consts::E;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use uuid::Uuid;

const FARADAY_CONSTANT: f64 = 96485.3321;
const MOLAR_MASS_H2: f64 = 2.01588e-3;
const HHV_H2: f64 = 286000.0;
const CELL_COUNT: f64 = 100.0;

pub struct EfficiencyModel {
    a: f64,
    b: f64,
    r: f64,
    exchange_current_density: f64,
    transfer_coefficient: f64,
}

impl Default for EfficiencyModel {
    fn default() -> Self {
        Self {
            a: 0.05,
            b: 0.03,
            r: 0.08,
            exchange_current_density: 1e-3,
            transfer_coefficient: 0.5,
        }
    }
}

impl EfficiencyModel {
    pub fn new(a: f64, b: f64, r: f64) -> Self {
        Self {
            a,
            b,
            r,
            exchange_current_density: 1e-3,
            transfer_coefficient: 0.5,
        }
    }

    pub fn calculate_polarization_voltage(&self, current_density: f64, temperature: f64) -> f64 {
        let temp_k = temperature + 273.15;
        let reversible_voltage = 1.229 - 0.0009 * (temp_k - 298.15);

        if current_density <= 0.0 {
            return reversible_voltage;
        }

        let activation_loss = self.a
            * (current_density / self.exchange_current_density)
                .ln()
                .max(0.0);

        let concentration_loss = self.b * (1.0 - E.powf(-current_density / self.b));

        let ohmic_loss = self.r * current_density;

        reversible_voltage + activation_loss + concentration_loss + ohmic_loss
    }

    pub fn calculate_voltage_efficiency(&self, current_density: f64, cell_voltage: f64) -> f64 {
        let thermoneutral_voltage = 1.481;
        if cell_voltage <= 0.0 {
            return 0.0;
        }
        (thermoneutral_voltage / cell_voltage) * 100.0
    }

    pub fn calculate_efficiency(
        &self,
        current_density: f64,
        cell_voltage: f64,
        temperature: f64,
    ) -> f64 {
        let polarization_voltage = self.calculate_polarization_voltage(current_density, temperature);
        let voltage_efficiency = self.calculate_voltage_efficiency(current_density, cell_voltage);
        
        let faradaic_efficiency = 95.0 + 5.0 / (1.0 + E.powf(-(current_density - 1.0) / 0.2));
        
        voltage_efficiency * faradaic_efficiency / 100.0
    }

    pub fn calculate_hydrogen_production_rate(
        &self,
        current_density: f64,
        active_area: f64,
    ) -> f64 {
        let current = current_density * active_area * 10000.0;
        let production_rate = (current * MOLAR_MASS_H2) / (2.0 * FARADAY_CONSTANT);
        production_rate * 3600.0
    }

    pub fn calculate_power_consumption(
        &self,
        current_density: f64,
        cell_voltage: f64,
        active_area: f64,
    ) -> f64 {
        let current = current_density * active_area * 10000.0;
        (current * cell_voltage * CELL_COUNT) / 1000.0
    }
}

pub struct GeneticAlgorithmOptimizer {
    population_size: usize,
    mutation_rate: f64,
    crossover_rate: f64,
    max_generations: u32,
    elitism_count: usize,
}

impl Default for GeneticAlgorithmOptimizer {
    fn default() -> Self {
        Self {
            population_size: 100,
            mutation_rate: 0.1,
            crossover_rate: 0.8,
            max_generations: 100,
            elitism_count: 5,
        }
    }
}

struct Individual {
    current_density: f64,
    water_temp: f64,
    fitness: f64,
}

impl GeneticAlgorithmOptimizer {
    pub fn new(
        population_size: usize,
        mutation_rate: f64,
        crossover_rate: f64,
        max_generations: u32,
    ) -> Self {
        Self {
            population_size,
            mutation_rate,
            crossover_rate,
            max_generations,
            elitism_count: 5,
        }
    }

    pub fn optimize(
        &self,
        model: &EfficiencyModel,
        current_cell_voltage: f64,
        min_current_density: f64,
        max_current_density: f64,
        min_temp: f64,
        max_temp: f64,
        target_efficiency: f64,
    ) -> OptimizationResult {
        let mut rng = rand::thread_rng();
        let mut population = self.initialize_population(
            &mut rng,
            min_current_density,
            max_current_density,
            min_temp,
            max_temp,
        );

        let mut best_fitness = f64::NEG_INFINITY;
        let mut best_individual: Option<Individual> = None;
        let mut generations_without_improvement = 0;

        for generation in 0..self.max_generations {
            for individual in &mut population {
                let cell_voltage = model.calculate_polarization_voltage(
                    individual.current_density,
                    individual.water_temp,
                );
                let efficiency = model.calculate_efficiency(
                    individual.current_density,
                    cell_voltage,
                    individual.water_temp,
                );
                
                let efficiency_penalty = if efficiency < target_efficiency {
                    (target_efficiency - efficiency) * 10.0
                } else {
                    0.0
                };
                
                let stability_penalty = (individual.current_density - 2.0).abs() * 2.0
                    + (individual.water_temp - 60.0).abs() * 0.5;

                individual.fitness = efficiency - efficiency_penalty - stability_penalty;

                if individual.fitness > best_fitness {
                    best_fitness = individual.fitness;
                    best_individual = Some(Individual {
                        current_density: individual.current_density,
                        water_temp: individual.water_temp,
                        fitness: individual.fitness,
                    });
                    generations_without_improvement = 0;
                }
            }

            generations_without_improvement += 1;
            if generations_without_improvement > 20 {
                debug!("Early stopping at generation {} due to no improvement", generation);
                break;
            }

            if generation % 10 == 0 {
                debug!(
                    "Generation {}: Best efficiency = {:.2}%, CD = {:.2}, Temp = {:.1}",
                    generation,
                    best_fitness.max(0.0),
                    best_individual.as_ref().map(|i| i.current_density).unwrap_or(0.0),
                    best_individual.as_ref().map(|i| i.water_temp).unwrap_or(0.0)
                );
            }

            population = self.selection(&population, &mut rng);
            population = self.crossover(&population, &mut rng);
            population = self.mutation(
                &population,
                &mut rng,
                min_current_density,
                max_current_density,
                min_temp,
                max_temp,
            );
        }

        let best = best_individual.unwrap_or(Individual {
            current_density: 2.0,
            water_temp: 60.0,
            fitness: best_fitness,
        });

        let expected_voltage = model.calculate_polarization_voltage(best.current_density, best.water_temp);
        let expected_efficiency = model.calculate_efficiency(best.current_density, expected_voltage, best.water_temp);

        OptimizationResult {
            params: OptimizationParams {
                current_density: best.current_density,
                water_temp: best.water_temp,
            },
            expected_efficiency,
            generations: self.max_generations,
            fitness: best.fitness,
        }
    }

    fn initialize_population(
        &self,
        rng: &mut impl Rng,
        min_cd: f64,
        max_cd: f64,
        min_temp: f64,
        max_temp: f64,
    ) -> Vec<Individual> {
        let mut population = Vec::with_capacity(self.population_size);
        
        for _ in 0..self.population_size {
            let cd = rng.gen_range(min_cd..=max_cd);
            let temp = rng.gen_range(min_temp..=max_temp);
            population.push(Individual {
                current_density: cd,
                water_temp: temp,
                fitness: 0.0,
            });
        }
        
        population
    }

    fn selection(&self, population: &[Individual], rng: &mut impl Rng) -> Vec<Individual> {
        let mut new_population = Vec::with_capacity(self.population_size);

        let mut sorted: Vec<&Individual> = population.iter().collect();
        sorted.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());

        for i in 0..self.elitism_count {
            new_population.push(Individual {
                current_density: sorted[i].current_density,
                water_temp: sorted[i].water_temp,
                fitness: sorted[i].fitness,
            });
        }

        let total_fitness: f64 = population.iter().map(|i| i.fitness.max(0.0)).sum();
        
        while new_population.len() < self.population_size {
            let mut cumulative = 0.0;
            let pick = rng.gen_range(0.0..total_fitness);
            
            for individual in population {
                cumulative += individual.fitness.max(0.0);
                if cumulative >= pick {
                    new_population.push(Individual {
                        current_density: individual.current_density,
                        water_temp: individual.water_temp,
                        fitness: 0.0,
                    });
                    break;
                }
            }
        }

        new_population
    }

    fn crossover(&self, population: &[Individual], rng: &mut impl Rng) -> Vec<Individual> {
        let mut new_population = Vec::with_capacity(self.population_size);

        for i in (0..self.population_size).step_by(2) {
            if i + 1 < self.population_size {
                let parent1 = &population[i];
                let parent2 = &population[i + 1];

                if rng.gen::<f64>() < self.crossover_rate {
                    let alpha = rng.gen::<f64>();
                    
                    let child1_cd = alpha * parent1.current_density + (1.0 - alpha) * parent2.current_density;
                    let child1_temp = alpha * parent1.water_temp + (1.0 - alpha) * parent2.water_temp;
                    
                    let child2_cd = (1.0 - alpha) * parent1.current_density + alpha * parent2.current_density;
                    let child2_temp = (1.0 - alpha) * parent1.water_temp + alpha * parent2.water_temp;

                    new_population.push(Individual {
                        current_density: child1_cd,
                        water_temp: child1_temp,
                        fitness: 0.0,
                    });
                    new_population.push(Individual {
                        current_density: child2_cd,
                        water_temp: child2_temp,
                        fitness: 0.0,
                    });
                } else {
                    new_population.push(Individual {
                        current_density: parent1.current_density,
                        water_temp: parent1.water_temp,
                        fitness: 0.0,
                    });
                    new_population.push(Individual {
                        current_density: parent2.current_density,
                        water_temp: parent2.water_temp,
                        fitness: 0.0,
                    });
                }
            }
        }

        new_population
    }

    fn mutation(
        &self,
        population: &[Individual],
        rng: &mut impl Rng,
        min_cd: f64,
        max_cd: f64,
        min_temp: f64,
        max_temp: f64,
    ) -> Vec<Individual> {
        let normal = Normal::new(0.0, 0.1).unwrap();

        population
            .iter()
            .map(|individual| {
                let mut new_cd = individual.current_density;
                let mut new_temp = individual.water_temp;

                if rng.gen::<f64>() < self.mutation_rate {
                    new_cd += normal.sample(rng) * (max_cd - min_cd);
                    new_cd = new_cd.clamp(min_cd, max_cd);
                }

                if rng.gen::<f64>() < self.mutation_rate {
                    new_temp += normal.sample(rng) * (max_temp - min_temp);
                    new_temp = new_temp.clamp(min_temp, max_temp);
                }

                Individual {
                    current_density: new_cd,
                    water_temp: new_temp,
                    fitness: 0.0,
                }
            })
            .collect()
    }
}

pub struct OptimizationService {
    pub model: EfficiencyModel,
    optimizer: GeneticAlgorithmOptimizer,
    pub min_current_density: f64,
    pub max_current_density: f64,
    pub min_temp: f64,
    pub max_temp: f64,
    pub efficiency_threshold: f64,
    pub target_efficiency: f64,
}

impl Default for OptimizationService {
    fn default() -> Self {
        Self {
            model: EfficiencyModel::default(),
            optimizer: GeneticAlgorithmOptimizer::default(),
            min_current_density: 0.5,
            max_current_density: 4.0,
            min_temp: 40.0,
            max_temp: 80.0,
            efficiency_threshold: 75.0,
            target_efficiency: 78.0,
        }
    }
}

impl OptimizationService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn calculate_current_efficiency(
        &self,
        current_density: f64,
        cell_voltage: f64,
        water_temp: f64,
    ) -> f64 {
        self.model.calculate_efficiency(current_density, cell_voltage, water_temp)
    }

    pub fn check_and_optimize(
        &self,
        electrolyzer_id: u8,
        current_density: f64,
        cell_voltage: f64,
        water_temp: f64,
    ) -> Option<OptimizationSuggestion> {
        let current_efficiency = self.calculate_current_efficiency(current_density, cell_voltage, water_temp);

        debug!(
            "Electrolyzer {}: Current efficiency = {:.2}% (threshold: {:.2}%)",
            electrolyzer_id, current_efficiency, self.efficiency_threshold
        );

        if current_efficiency >= self.efficiency_threshold {
            return None;
        }

        warn!(
            "Electrolyzer {} efficiency {:.2}% below threshold {:.2}%, starting optimization...",
            electrolyzer_id, current_efficiency, self.efficiency_threshold
        );

        let result = self.optimizer.optimize(
            &self.model,
            cell_voltage,
            self.min_current_density,
            self.max_current_density,
            self.min_temp,
            self.max_temp,
            self.target_efficiency,
        );

        if result.expected_efficiency >= self.target_efficiency {
            info!(
                "Optimization found for electrolyzer {}: CD {:.2} -> {:.2} A/cm², Temp {:.1} -> {:.1}°C, Expected efficiency: {:.2}%",
                electrolyzer_id,
                current_density,
                result.params.current_density,
                water_temp,
                result.params.water_temp,
                result.expected_efficiency
            );

            Some(OptimizationSuggestion {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                electrolyzer_id,
                current_efficiency,
                optimized_current_density: result.params.current_density,
                optimized_water_temp: result.params.water_temp,
                expected_efficiency: result.expected_efficiency,
                applied: false,
            })
        } else {
            warn!(
                "Could not find optimization solution for electrolyzer {} that meets target efficiency",
                electrolyzer_id
            );
            None
        }
    }

    pub fn get_efficiency_curve(
        &self,
        current_density_range: std::ops::Range<f64>,
        steps: usize,
        water_temp: f64,
    ) -> Vec<(f64, f64)> {
        let mut curve = Vec::with_capacity(steps);
        let step = (current_density_range.end - current_density_range.start) / steps as f64;

        for i in 0..=steps {
            let cd = current_density_range.start + step * i as f64;
            let voltage = self.model.calculate_polarization_voltage(cd, water_temp);
            let efficiency = self.model.calculate_efficiency(cd, voltage, water_temp);
            curve.push((cd, efficiency));
        }

        curve
    }

    pub fn get_polarization_curve(
        &self,
        current_density_range: std::ops::Range<f64>,
        steps: usize,
        water_temp: f64,
    ) -> Vec<(f64, f64)> {
        let mut curve = Vec::with_capacity(steps);
        let step = (current_density_range.end - current_density_range.start) / steps as f64;

        for i in 0..=steps {
            let cd = current_density_range.start + step * i as f64;
            let voltage = self.model.calculate_polarization_voltage(cd, water_temp);
            curve.push((cd, voltage));
        }

        curve
    }
}

#[derive(Debug, Clone)]
pub struct OptimizationTask {
    pub electrolyzer_id: u8,
    pub current_density: f64,
    pub cell_voltage: f64,
    pub water_temp: f64,
    pub current_efficiency: f64,
    pub timestamp: DateTime<Utc>,
}

pub struct OptimizationQueueHandle {
    task_tx: mpsc::Sender<OptimizationTask>,
    result_rx: Option<mpsc::Receiver<OptimizationSuggestion>>,
    pending_electrolyzers: Arc<std::sync::Mutex<HashSet<u8>>>,
}

impl OptimizationQueueHandle {
    pub async fn submit_optimization(
        &self,
        task: OptimizationTask,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut pending = self.pending_electrolyzers.lock().unwrap();
        
        if pending.contains(&task.electrolyzer_id) {
            debug!(
                "Optimization for electrolyzer {} already in queue, skipping",
                task.electrolyzer_id
            );
            return Ok(());
        }
        
        pending.insert(task.electrolyzer_id);
        drop(pending);
        
        if let Err(e) = self.task_tx.send(task).await {
            let mut pending = self.pending_electrolyzers.lock().unwrap();
            pending.remove(&e.0.electrolyzer_id);
            return Err(format!("Failed to submit optimization task: {}", e).into());
        }
        
        Ok(())
    }

    pub fn poll_result(&mut self) -> Option<OptimizationSuggestion> {
        self.result_rx.as_mut().and_then(|rx| rx.try_recv().ok())
    }
}

pub struct GlobalOptimizationQueue {
    optimization_service: Arc<OptimizationService>,
    task_rx: mpsc::Receiver<OptimizationTask>,
    result_tx: mpsc::Sender<OptimizationSuggestion>,
    pending_electrolyzers: Arc<std::sync::Mutex<HashSet<u8>>>,
    concurrency_semaphore: Arc<Semaphore>,
    max_concurrent_tasks: usize,
}

impl GlobalOptimizationQueue {
    pub fn new(
        optimization_service: Arc<OptimizationService>,
        max_concurrent_tasks: usize,
        queue_capacity: usize,
    ) -> (Self, OptimizationQueueHandle) {
        let (task_tx, task_rx) = mpsc::channel::<OptimizationTask>(queue_capacity);
        let (result_tx, result_rx) = mpsc::channel::<OptimizationSuggestion>(queue_capacity);
        let pending_electrolyzers = Arc::new(std::sync::Mutex::new(HashSet::new()));

        let handle = OptimizationQueueHandle {
            task_tx,
            result_rx: Some(result_rx),
            pending_electrolyzers: pending_electrolyzers.clone(),
        };

        let queue = Self {
            optimization_service,
            task_rx,
            result_tx,
            pending_electrolyzers,
            concurrency_semaphore: Arc::new(Semaphore::new(max_concurrent_tasks)),
            max_concurrent_tasks,
        };

        (queue, handle)
    }

    pub async fn run(mut self) {
        info!(
            "Global optimization queue started with max {} concurrent tasks",
            self.max_concurrent_tasks
        );

        while let Some(task) = self.task_rx.recv().await {
            let semaphore = self.concurrency_semaphore.clone();
            let service = self.optimization_service.clone();
            let result_tx = self.result_tx.clone();
            let pending = self.pending_electrolyzers.clone();
            let electrolyzer_id = task.electrolyzer_id;

            tokio::spawn(async move {
                let _permit = match semaphore.acquire().await {
                    Ok(permit) => permit,
                    Err(e) => {
                        error!("Semaphore acquire error: {}", e);
                        let mut pending = pending.lock().unwrap();
                        pending.remove(&electrolyzer_id);
                        return;
                    }
                };

                debug!(
                    "Starting optimization for electrolyzer {} (efficiency: {:.2}%)",
                    electrolyzer_id, task.current_efficiency
                );

                let start_time = std::time::Instant::now();
                
                let result = tokio::task::spawn_blocking(move || {
                    service.check_and_optimize(
                        task.electrolyzer_id,
                        task.current_density,
                        task.cell_voltage,
                        task.water_temp,
                    )
                })
                .await;

                let elapsed = start_time.elapsed();

                let mut pending = pending.lock().unwrap();
                pending.remove(&electrolyzer_id);
                drop(pending);

                match result {
                    Ok(Some(suggestion)) => {
                        info!(
                            "Optimization completed for electrolyzer {} in {:.2?}, expected efficiency: {:.2}%",
                            electrolyzer_id, elapsed, suggestion.expected_efficiency
                        );
                        
                        if let Err(e) = result_tx.send(suggestion).await {
                            error!("Failed to send optimization result: {}", e);
                        }
                    }
                    Ok(None) => {
                        debug!(
                            "No optimization needed for electrolyzer {} (completed in {:.2?})",
                            electrolyzer_id, elapsed
                        );
                    }
                    Err(e) => {
                        error!(
                            "Optimization task failed for electrolyzer {}: {}",
                            electrolyzer_id, e
                        );
                    }
                }
            });
        }

        warn!("Global optimization queue task receiver closed");
    }

    pub fn get_queue_stats(&self) -> OptimizationQueueStats {
        OptimizationQueueStats {
            pending_tasks: self.task_rx.len(),
            active_tasks: self.max_concurrent_tasks - self.concurrency_semaphore.available_permits(),
            max_concurrent: self.max_concurrent_tasks,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OptimizationQueueStats {
    pub pending_tasks: usize,
    pub active_tasks: usize,
    pub max_concurrent: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polarization_curve() {
        let model = EfficiencyModel::default();
        let voltage = model.calculate_polarization_voltage(2.0, 60.0);
        assert!(voltage > 1.0 && voltage < 2.5);
    }

    #[test]
    fn test_efficiency_calculation() {
        let model = EfficiencyModel::default();
        let efficiency = model.calculate_efficiency(2.0, 1.85, 60.0);
        assert!(efficiency > 70.0 && efficiency < 95.0);
    }

    #[test]
    fn test_optimization() {
        let optimizer = GeneticAlgorithmOptimizer::new(50, 0.1, 0.8, 50);
        let model = EfficiencyModel::default();
        
        let result = optimizer.optimize(
            &model,
            2.0,
            0.5,
            4.0,
            40.0,
            80.0,
            78.0,
        );
        
        assert!(result.params.current_density >= 0.5 && result.params.current_density <= 4.0);
        assert!(result.params.water_temp >= 40.0 && result.params.water_temp <= 80.0);
        assert!(result.expected_efficiency > 70.0);
    }
}
