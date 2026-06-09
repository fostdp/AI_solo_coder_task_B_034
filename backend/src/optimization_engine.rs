use crate::config::{GeneticAlgorithmConfig, OptimizationConfig};
use crate::efficiency_analyzer::{EfficiencyModel, OptimizationResultMessage, OptimizationTask};
use crate::models::*;
use chrono::Utc;
use log::{debug, error, info, warn};
use rand::Rng;
use rand_distr::{Distribution, Normal};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use uuid::Uuid;

struct Individual {
    current_density: f64,
    water_temp: f64,
    fitness: f64,
}

#[derive(Debug, Clone)]
struct OptimizationResult {
    params: OptimizationParams,
    expected_efficiency: f64,
    generations: u32,
    fitness: f64,
}

#[derive(Debug, Clone)]
struct OptimizationParams {
    current_density: f64,
    water_temp: f64,
}

#[derive(Clone)]
pub struct GeneticAlgorithmOptimizer {
    config: GeneticAlgorithmConfig,
}

impl GeneticAlgorithmOptimizer {
    pub fn new(config: GeneticAlgorithmConfig) -> Self {
        Self { config }
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

        for generation in 0..self.config.max_generations {
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
            generations: self.config.max_generations,
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
        let mut population = Vec::with_capacity(self.config.population_size);
        
        for _ in 0..self.config.population_size {
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
        let mut new_population = Vec::with_capacity(self.config.population_size);

        let mut sorted: Vec<&Individual> = population.iter().collect();
        sorted.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());

        for i in 0..self.config.elitism_count {
            new_population.push(Individual {
                current_density: sorted[i].current_density,
                water_temp: sorted[i].water_temp,
                fitness: sorted[i].fitness,
            });
        }

        let total_fitness: f64 = population.iter().map(|i| i.fitness.max(0.0)).sum();
        
        while new_population.len() < self.config.population_size {
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
        let mut new_population = Vec::with_capacity(self.config.population_size);

        for i in (0..self.config.population_size).step_by(2) {
            if i + 1 < self.config.population_size {
                let parent1 = &population[i];
                let parent2 = &population[i + 1];

                if rng.gen::<f64>() < self.config.crossover_rate {
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

                if rng.gen::<f64>() < self.config.mutation_rate {
                    new_cd += normal.sample(rng) * (max_cd - min_cd);
                    new_cd = new_cd.clamp(min_cd, max_cd);
                }

                if rng.gen::<f64>() < self.config.mutation_rate {
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

pub struct OptimizationEngine {
    model: Arc<EfficiencyModel>,
    optimizer: GeneticAlgorithmOptimizer,
    optimization_config: OptimizationConfig,
    task_rx: mpsc::Receiver<OptimizationTask>,
    result_tx: mpsc::Sender<OptimizationResultMessage>,
    pending_electrolyzers: Arc<std::sync::Mutex<HashSet<u8>>>,
    concurrency_semaphore: Arc<Semaphore>,
    max_concurrent_tasks: usize,
}

impl OptimizationEngine {
    pub fn new(
        model: Arc<EfficiencyModel>,
        ga_config: GeneticAlgorithmConfig,
        optimization_config: OptimizationConfig,
    ) -> (Self, crate::efficiency_analyzer::OptimizationEngineHandle) {
        let (task_tx, task_rx) = mpsc::channel::<OptimizationTask>(optimization_config.queue_capacity);
        let (result_tx, result_rx) = mpsc::channel::<OptimizationResultMessage>(optimization_config.queue_capacity);
        let pending_electrolyzers = Arc::new(std::sync::Mutex::new(HashSet::new()));

        let handle = crate::efficiency_analyzer::OptimizationEngineHandle {
            task_tx,
            result_rx: Some(result_rx),
            pending_electrolyzers: pending_electrolyzers.clone(),
        };

        let engine = Self {
            model,
            optimizer: GeneticAlgorithmOptimizer::new(ga_config),
            optimization_config: optimization_config.clone(),
            task_rx,
            result_tx,
            pending_electrolyzers,
            concurrency_semaphore: Arc::new(Semaphore::new(optimization_config.max_concurrent_optimizations)),
            max_concurrent_tasks: optimization_config.max_concurrent_optimizations,
        };

        (engine, handle)
    }

    pub async fn run(mut self) {
        info!(
            "Optimization engine started with max {} concurrent tasks",
            self.max_concurrent_tasks
        );

        while let Some(task) = self.task_rx.recv().await {
            let semaphore = self.concurrency_semaphore.clone();
            let model = self.model.clone();
            let optimizer = self.optimizer.clone();
            let result_tx = self.result_tx.clone();
            let pending = self.pending_electrolyzers.clone();
            let electrolyzer_id = task.electrolyzer_id;
            let opt_config = self.optimization_config.clone();

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
                    let result = optimizer.optimize(
                        &model,
                        task.cell_voltage,
                        opt_config.min_current_density,
                        opt_config.max_current_density,
                        opt_config.min_temp,
                        opt_config.max_temp,
                        opt_config.target_efficiency,
                    );

                    if result.expected_efficiency >= opt_config.target_efficiency {
                        Some(OptimizationSuggestion {
                            id: Uuid::new_v4(),
                            timestamp: Utc::now(),
                            electrolyzer_id,
                            current_efficiency: task.current_efficiency,
                            optimized_current_density: result.params.current_density,
                            optimized_water_temp: result.params.water_temp,
                            expected_efficiency: result.expected_efficiency,
                            applied: false,
                        })
                    } else {
                        None
                    }
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
                        
                        if let Err(e) = result_tx.send(OptimizationResultMessage { suggestion }).await {
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

        warn!("Optimization engine task receiver closed");
    }

    pub fn get_queue_stats(&self) -> OptimizationQueueStats {
        OptimizationQueueStats {
            pending_tasks: self.task_rx.len(),
            active_tasks: self.max_concurrent_tasks - self.concurrency_semaphore.available_permits(),
            max_concurrent: self.max_concurrent_tasks,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OptimizationQueueStats {
    pub pending_tasks: usize,
    pub active_tasks: usize,
    pub max_concurrent: usize,
}
