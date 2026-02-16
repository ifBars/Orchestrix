//! Business simulation engine for LLM benchmarking.
//!
//! Provides deterministic profit-driven scenarios where models act as operators
//! of a fictional vending company with tools for supplier negotiations,
//! supply purchases, machine restocking, and pricing decisions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Scenario data structures -- deserialize from JSON
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub scenario_id: String,
    pub name: String,
    pub description: String,
    pub seed: u64,
    pub horizon_turns: usize,
    pub initial_state: InitialState,
    pub suppliers: Vec<Supplier>,
    pub demand_model: DemandModel,
    pub events: Vec<ScheduledEvent>,
    pub constraints: Constraints,
    pub scoring: ScoringWeights,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitialState {
    pub cash: f64,
    pub trust: i32,
    pub machines: Vec<Machine>,
    pub inventory: Vec<InventoryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Machine {
    pub machine_id: String,
    pub location_tier: String,
    pub capacity: usize,
    pub uptime: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItem {
    pub sku: String,
    pub qty: usize,
    pub unit_cost: f64,
    pub shelf_life_days: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Supplier {
    pub supplier_id: String,
    pub reliability: f64,
    pub lead_time_days: usize,
    pub min_order_qty: usize,
    pub catalog: Vec<CatalogItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogItem {
    pub sku: String,
    pub base_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemandModel {
    pub base_daily_demand_per_machine: f64,
    pub seasonality: Vec<f64>,
    pub price_elasticity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledEvent {
    pub turn: usize,
    pub event_type: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraints {
    pub max_email_per_turn: usize,
    pub max_tool_calls_per_turn: usize,
    pub min_cash_floor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub profit_weight: f64,
    pub service_level_weight: f64,
    pub solvency_weight: f64,
    pub compliance_weight: f64,
}

// ---------------------------------------------------------------------------
// Simulator runtime state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Simulator {
    pub scenario: Scenario,
    pub state: SimulatorState,
    pub turn: usize,
    rng: SimpleRng,
    pending_orders: Vec<PendingOrder>,
    email_log: Vec<EmailLogEntry>,
    violation_count: usize,
    tool_calls_this_turn: usize,
    emails_sent_this_turn: usize,
    // Track demand fulfillment for accurate service level calculation
    total_demand: usize,
    fulfilled_demand: usize,
    stockout_turns: usize,
    total_turns_tracked: usize,
}

#[derive(Debug, Clone)]
pub struct SimulatorState {
    pub cash: f64,
    pub trust: i32,
    pub machines: HashMap<String, MachineState>,
    pub inventory: HashMap<String, InventoryState>,
    pub contracts: HashMap<String, Contract>,
}

#[derive(Debug, Clone)]
pub struct MachineState {
    pub machine_id: String,
    pub location_tier: String,
    pub capacity: usize,
    pub uptime: f64,
    pub stock: HashMap<String, usize>,
    pub prices: HashMap<String, f64>,
    pub maintenance_pending: bool,
}

#[derive(Debug, Clone)]
pub struct InventoryState {
    pub sku: String,
    pub qty: usize,
    pub unit_cost: f64,
    pub shelf_life_days: usize,
}

#[derive(Debug, Clone)]
pub struct Contract {
    #[allow(dead_code)]
    pub supplier_id: String,
    #[allow(dead_code)]
    pub sku: String,
    pub unit_price: f64,
    pub min_qty: usize,
    pub lead_time_days: usize,
}

#[derive(Debug, Clone)]
pub struct PendingOrder {
    pub supplier_id: String,
    pub sku: String,
    pub qty: usize,
    pub unit_price: f64,
    pub delivery_turn: usize,
}

#[derive(Debug, Clone)]
pub struct EmailLogEntry {
    pub turn: usize,
    pub supplier_id: String,
    pub subject: String,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalScore {
    pub raw_profit: f64,
    pub normalized_profit: f64,
    pub service_level: f64,
    pub solvency_score: f64,
    pub compliance_score: f64,
    pub weighted_score: f64,
    pub stockout_rate: f64,
    pub final_cash: f64,
    pub min_cash: f64,
    pub bankrupt_turn: Option<usize>,
    pub total_emails_sent: usize,
}

// ---------------------------------------------------------------------------
// Deterministic RNG
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_mul(1103515245).wrapping_add(12345),
        }
    }

    fn next(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(1103515245).wrapping_add(12345);
        (self.state >> 16) as u32
    }
}

// ---------------------------------------------------------------------------
// Simulator implementation
// ---------------------------------------------------------------------------

impl Simulator {
    pub fn new(scenario: Scenario) -> Self {
        let rng = SimpleRng::new(scenario.seed);
        let initial_trust = scenario.initial_state.trust;
        let initial_cash = scenario.initial_state.cash;

        let mut machines = HashMap::new();
        for m in &scenario.initial_state.machines {
            let stock: HashMap<String, usize> = scenario
                .initial_state
                .inventory
                .iter()
                .map(|inv| (inv.sku.clone(), 0))
                .collect();
            let prices: HashMap<String, f64> = scenario
                .initial_state
                .inventory
                .iter()
                .map(|inv| (inv.sku.clone(), 2.0))
                .collect();

            machines.insert(
                m.machine_id.clone(),
                MachineState {
                    machine_id: m.machine_id.clone(),
                    location_tier: m.location_tier.clone(),
                    capacity: m.capacity,
                    uptime: m.uptime,
                    stock,
                    prices,
                    maintenance_pending: false,
                },
            );
        }

        let mut inventory = HashMap::new();
        for inv in &scenario.initial_state.inventory {
            inventory.insert(
                inv.sku.clone(),
                InventoryState {
                    sku: inv.sku.clone(),
                    qty: inv.qty,
                    unit_cost: inv.unit_cost,
                    shelf_life_days: inv.shelf_life_days,
                },
            );
        }

        Self {
            scenario,
            state: SimulatorState {
                cash: initial_cash,
                trust: initial_trust,
                machines,
                inventory,
                contracts: HashMap::new(),
            },
            turn: 0,
            rng,
            pending_orders: Vec::new(),
            email_log: Vec::new(),
            violation_count: 0,
            tool_calls_this_turn: 0,
            emails_sent_this_turn: 0,
            // Initialize demand tracking
            total_demand: 0,
            fulfilled_demand: 0,
            stockout_turns: 0,
            total_turns_tracked: 0,
        }
    }

    pub fn current_state_json(&self) -> String {
        let machines: Vec<_> = self
            .state
            .machines
            .values()
            .map(|m| {
                serde_json::json!({
                    "machine_id": m.machine_id,
                    "location_tier": m.location_tier,
                    "capacity": m.capacity,
                    "uptime": m.uptime,
                    "stock": m.stock,
                    "prices": m.prices,
                    "maintenance_pending": m.maintenance_pending,
                })
            })
            .collect();

        let inventory: Vec<_> = self
            .state
            .inventory
            .values()
            .map(|inv| {
                serde_json::json!({
                    "sku": inv.sku,
                    "qty": inv.qty,
                    "unit_cost": inv.unit_cost,
                })
            })
            .collect();

        let suppliers: Vec<_> = self
            .scenario
            .suppliers
            .iter()
            .map(|s| {
                serde_json::json!({
                    "supplier_id": s.supplier_id,
                    "reliability": s.reliability,
                    "lead_time_days": s.lead_time_days,
                    "min_order_qty": s.min_order_qty,
                    "catalog": s.catalog,
                })
            })
            .collect();

        let contracts: Vec<_> = self
            .state
            .contracts
            .values()
            .map(|c| {
                serde_json::json!({
                    "supplier_id": c.supplier_id,
                    "sku": c.sku,
                    "unit_price": c.unit_price,
                    "min_qty": c.min_qty,
                    "lead_time_days": c.lead_time_days,
                })
            })
            .collect();

        let pending: Vec<_> = self
            .pending_orders
            .iter()
            .map(|o| {
                serde_json::json!({
                    "supplier_id": o.supplier_id,
                    "sku": o.sku,
                    "qty": o.qty,
                    "unit_price": o.unit_price,
                    "delivery_turn": o.delivery_turn,
                })
            })
            .collect();

        let event_info: Vec<String> = self
            .scenario
            .events
            .iter()
            .filter(|e| e.turn > self.turn && e.turn <= self.turn + 3)
            .map(|e| format!("{} (turn {})", e.event_type, e.turn))
            .collect();

        // Calculate key metrics for the model
        let total_inventory: usize = self.state.inventory.values().map(|inv| inv.qty).sum();
        let total_machine_stock: usize = self
            .state
            .machines
            .values()
            .map(|m| m.stock.values().sum::<usize>())
            .sum();
        let any_stockouts = self
            .state
            .machines
            .values()
            .any(|m| m.stock.values().any(|qty| *qty == 0));

        // Demand forecast summary
        let season_idx = self.turn % self.scenario.demand_model.seasonality.len();
        let current_season = self.scenario.demand_model.seasonality[season_idx];
        let base_demand = self.scenario.demand_model.base_daily_demand_per_machine;

        serde_json::json!({
            "turn": self.turn + 1,
            "total_turns": self.scenario.horizon_turns,
            "cash": format!("{:.2}", self.state.cash),
            "trust": self.state.trust,
            "machines": machines,
            "inventory": inventory,
            "suppliers": suppliers,
            "contracts": contracts,
            "pending_orders": pending,
            "upcoming_events": event_info,
            // Summary metrics to reduce need for view_reports calls
            "summary": {
                "total_warehouse_stock": total_inventory,
                "total_machine_stock": total_machine_stock,
                "any_stockouts": any_stockouts,
                "daily_demand_estimate": format!("{:.0}", base_demand * current_season),
                "price_elasticity": self.scenario.demand_model.price_elasticity,
            }
        })
        .to_string()
    }

    pub fn view_report(&self, report_type: &str) -> String {
        match report_type {
            "cashflow" => {
                let total_inventory_value: f64 = self
                    .state
                    .inventory
                    .values()
                    .map(|inv| inv.qty as f64 * inv.unit_cost)
                    .sum();
                serde_json::json!({
                    "type": "cashflow",
                    "current_cash": format!("{:.2}", self.state.cash),
                    "inventory_value": format!("{:.2}", total_inventory_value),
                    "total_assets": format!("{:.2}", self.state.cash + total_inventory_value),
                })
                .to_string()
            }
            "demand_forecast" => {
                let season_idx = self.turn % self.scenario.demand_model.seasonality.len();
                let season = self.scenario.demand_model.seasonality[season_idx];
                serde_json::json!({
                    "type": "demand_forecast",
                    "current_seasonality": season,
                    "base_daily_demand": self.scenario.demand_model.base_daily_demand_per_machine,
                    "price_elasticity": self.scenario.demand_model.price_elasticity,
                })
                .to_string()
            }
            "stockouts" => {
                let stockouts: Vec<_> = self
                    .state
                    .machines
                    .values()
                    .flat_map(|m| {
                        m.stock
                            .iter()
                            .filter(|(_, qty)| **qty == 0)
                            .map(|(sku, _)| format!("{}@{}", m.machine_id, sku))
                    })
                    .collect();
                serde_json::json!({
                    "type": "stockouts",
                    "stockouts": stockouts,
                    "count": stockouts.len(),
                })
                .to_string()
            }
            "machine_health" => {
                let health: Vec<_> = self
                    .state
                    .machines
                    .values()
                    .map(|m| {
                        let utilization = if m.capacity > 0 {
                            m.stock.values().sum::<usize>() as f64 / m.capacity as f64
                        } else {
                            0.0
                        };
                        serde_json::json!({
                            "machine_id": m.machine_id,
                            "uptime": m.uptime,
                            "maintenance_pending": m.maintenance_pending,
                            "utilization": format!("{:.2}", utilization),
                        })
                    })
                    .collect();
                serde_json::json!({
                    "type": "machine_health",
                    "machines": health,
                })
                .to_string()
            }
            "supplier_sla" => {
                let sla: Vec<_> = self
                    .scenario
                    .suppliers
                    .iter()
                    .map(|s| {
                        let has_contract = self
                            .state
                            .contracts
                            .values()
                            .any(|c| c.supplier_id == s.supplier_id);
                        serde_json::json!({
                            "supplier_id": s.supplier_id,
                            "reliability": s.reliability,
                            "lead_time_days": s.lead_time_days,
                            "has_contract": has_contract,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "type": "supplier_sla",
                    "suppliers": sla,
                })
                .to_string()
            }
            _ => serde_json::json!({"error": "unknown report type"}).to_string(),
        }
    }

    pub fn tool_call(&mut self, tool_name: &str, args: &serde_json::Value) -> ToolResult {
        self.tool_calls_this_turn += 1;

        if self.tool_calls_this_turn > self.scenario.constraints.max_tool_calls_per_turn {
            self.violation_count += 1;
            return ToolResult {
                success: false,
                message: "exceeded max tool calls per turn".to_string(),
            };
        }

        match tool_name {
            "email_supplier" => self.handle_email_supplier(args),
            "negotiate_contract" => self.handle_negotiate_contract(args),
            "purchase_supply" => self.handle_purchase_supply(args),
            "restock_machine" => self.handle_restock_machine(args),
            "set_price" => self.handle_set_price(args),
            "schedule_maintenance" => self.handle_schedule_maintenance(args),
            "view_reports" => self.handle_view_report(args),
            "end_turn" => self.handle_end_turn(),
            _ => ToolResult {
                success: false,
                message: format!("unknown tool: {}", tool_name),
            },
        }
    }

    fn handle_email_supplier(&mut self, args: &serde_json::Value) -> ToolResult {
        let supplier_id = args
            .get("supplier_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let subject = args.get("subject").and_then(|v| v.as_str()).unwrap_or("");

        if supplier_id.is_empty() {
            return ToolResult {
                success: false,
                message: "supplier_id required".to_string(),
            };
        }

        self.emails_sent_this_turn += 1;
        if self.emails_sent_this_turn > self.scenario.constraints.max_email_per_turn {
            self.violation_count += 1;
            return ToolResult {
                success: false,
                message: "exceeded max emails per turn".to_string(),
            };
        }

        self.email_log.push(EmailLogEntry {
            turn: self.turn,
            supplier_id: supplier_id.to_string(),
            subject: subject.to_string(),
        });

        let supplier = self
            .scenario
            .suppliers
            .iter()
            .find(|s| s.supplier_id == supplier_id);

        let reply = match supplier {
            Some(s) => {
                let rng_val = self.rng.next() % 100;
                if rng_val < (s.reliability * 100.0) as u32 {
                    let delay = 1 + (self.rng.next() % 2);
                    format!(
                        "thank you for your inquiry we can offer {} day response time",
                        delay + s.lead_time_days as u32
                    )
                } else {
                    "currently reviewing your request will respond in 3-5 business days".to_string()
                }
            }
            None => format!("supplier '{}' not found", supplier_id),
        };

        ToolResult {
            success: true,
            message: reply,
        }
    }

    fn handle_negotiate_contract(&mut self, args: &serde_json::Value) -> ToolResult {
        let supplier_id = args
            .get("supplier_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let sku = args.get("sku").and_then(|v| v.as_str()).unwrap_or("water");
        let target_price = args
            .get("target_price")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.50);
        let min_qty = args.get("min_qty").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

        let supplier = self
            .scenario
            .suppliers
            .iter()
            .find(|s| s.supplier_id == supplier_id);

        match supplier {
            Some(s) => {
                let catalog_item = s.catalog.iter().find(|c| c.sku == sku);
                let base_price = catalog_item.map(|c| c.base_price).unwrap_or(0.50);

                let final_price = if target_price < base_price * 0.85 {
                    base_price * 0.92
                } else {
                    (base_price + target_price) / 2.0
                };

                let lead_time = if min_qty > s.min_order_qty * 2 {
                    s.lead_time_days.saturating_sub(1)
                } else {
                    s.lead_time_days
                };

                self.state.contracts.insert(
                    format!("{}_{}", supplier_id, sku),
                    Contract {
                        supplier_id: supplier_id.to_string(),
                        sku: sku.to_string(),
                        unit_price: final_price,
                        min_qty,
                        lead_time_days: lead_time,
                    },
                );

                ToolResult {
                    success: true,
                    message: format!(
                        "contract negotiated: {} {} @ ${:.2} (min {} units {} day lead)",
                        supplier_id, sku, final_price, min_qty, lead_time
                    ),
                }
            }
            None => ToolResult {
                success: false,
                message: format!("supplier '{}' not found", supplier_id),
            },
        }
    }

    fn handle_purchase_supply(&mut self, args: &serde_json::Value) -> ToolResult {
        let supplier_id = args
            .get("supplier_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let sku = args.get("sku").and_then(|v| v.as_str()).unwrap_or("water");
        let qty = args.get("qty").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

        if qty == 0 {
            return ToolResult {
                success: false,
                message: "qty must be positive".to_string(),
            };
        }

        let contract = self
            .state
            .contracts
            .get(&format!("{}_{}", supplier_id, sku));

        let unit_price = match contract {
            Some(c) if qty >= c.min_qty => c.unit_price,
            _ => {
                let supplier = self
                    .scenario
                    .suppliers
                    .iter()
                    .find(|s| s.supplier_id == supplier_id);
                supplier
                    .and_then(|s| s.catalog.iter().find(|c| c.sku == sku))
                    .map(|c| c.base_price)
                    .unwrap_or(0.55)
            }
        };

        let total_cost = unit_price * qty as f64;

        if total_cost > self.state.cash {
            return ToolResult {
                success: false,
                message: format!(
                    "insufficient cash: need ${:.2} have ${:.2}",
                    total_cost, self.state.cash
                ),
            };
        }

        self.state.cash -= total_cost;

        let lead_time = contract.map(|c| c.lead_time_days).unwrap_or(5);
        let delivery_turn = self.turn + lead_time;

        self.pending_orders.push(PendingOrder {
            supplier_id: supplier_id.to_string(),
            sku: sku.to_string(),
            qty,
            unit_price,
            delivery_turn,
        });

        ToolResult {
            success: true,
            message: format!(
                "order placed: {} {} from {} (${:.2}) arriving turn {}",
                qty, sku, supplier_id, total_cost, delivery_turn
            ),
        }
    }

    fn handle_restock_machine(&mut self, args: &serde_json::Value) -> ToolResult {
        let machine_id = args
            .get("machine_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let sku = args.get("sku").and_then(|v| v.as_str()).unwrap_or("water");
        let qty = args.get("qty").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

        let machine = match self.state.machines.get_mut(machine_id) {
            Some(m) => m,
            None => {
                return ToolResult {
                    success: false,
                    message: format!("machine '{}' not found", machine_id),
                }
            }
        };

        let inv = match self.state.inventory.get_mut(sku) {
            Some(i) => i,
            None => {
                return ToolResult {
                    success: false,
                    message: format!("sku '{}' not in inventory", sku),
                }
            }
        };

        if inv.qty < qty {
            return ToolResult {
                success: false,
                message: format!("insufficient inventory: have {} need {}", inv.qty, qty),
            };
        }

        let current_stock = machine.stock.get(sku).copied().unwrap_or(0);
        let space_remaining = machine.capacity.saturating_sub(current_stock);
        let actual_qty = qty.min(space_remaining);

        if actual_qty == 0 {
            return ToolResult {
                success: false,
                message: format!("machine {} at full capacity for {}", machine_id, sku),
            };
        }

        inv.qty -= actual_qty;
        *machine.stock.entry(sku.to_string()).or_insert(0) += actual_qty;

        ToolResult {
            success: true,
            message: format!("restocked {} {} to machine {}", actual_qty, sku, machine_id),
        }
    }

    fn handle_set_price(&mut self, args: &serde_json::Value) -> ToolResult {
        let machine_id = args
            .get("machine_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let sku = args.get("sku").and_then(|v| v.as_str()).unwrap_or("water");
        let price = args
            .get("unit_price")
            .and_then(|v| v.as_f64())
            .unwrap_or(2.0);

        let machine = match self.state.machines.get_mut(machine_id) {
            Some(m) => m,
            None => {
                return ToolResult {
                    success: false,
                    message: format!("machine '{}' not found", machine_id),
                }
            }
        };

        machine.prices.insert(sku.to_string(), price);

        ToolResult {
            success: true,
            message: format!(
                "set {} price to ${:.2} at machine {}",
                sku, price, machine_id
            ),
        }
    }

    fn handle_schedule_maintenance(&mut self, args: &serde_json::Value) -> ToolResult {
        let machine_id = args
            .get("machine_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let machine = match self.state.machines.get_mut(machine_id) {
            Some(m) => m,
            None => {
                return ToolResult {
                    success: false,
                    message: format!("machine '{}' not found", machine_id),
                }
            }
        };

        let cost = 100.0;
        if self.state.cash < cost {
            return ToolResult {
                success: false,
                message: format!("insufficient cash for maintenance: need ${}", cost),
            };
        }

        self.state.cash -= cost;
        machine.maintenance_pending = true;

        ToolResult {
            success: true,
            message: format!(
                "maintenance scheduled for machine {} (${})",
                machine_id, cost
            ),
        }
    }

    fn handle_view_report(&self, args: &serde_json::Value) -> ToolResult {
        let report_type = args
            .get("report_type")
            .and_then(|v| v.as_str())
            .unwrap_or("cashflow");

        ToolResult {
            success: true,
            message: self.view_report(report_type),
        }
    }

    fn handle_end_turn(&mut self) -> ToolResult {
        self.process_turn();
        self.tool_calls_this_turn = 0;
        self.emails_sent_this_turn = 0;

        ToolResult {
            success: true,
            message: format!(
                "turn {} completed cash ${:.2} trust {}",
                self.turn, self.state.cash, self.state.trust
            ),
        }
    }

    fn process_turn(&mut self) {
        self.turn += 1;

        // Process deliveries - remove orders that have arrived
        let mut delivered_indices = Vec::new();
        for (i, order) in self.pending_orders.iter().enumerate() {
            if order.delivery_turn == self.turn {
                delivered_indices.push(i);
            }
        }

        // Process from the end to maintain index validity
        for &i in delivered_indices.iter().rev() {
            let order = self.pending_orders.remove(i);
            let inv = self
                .state
                .inventory
                .entry(order.sku.clone())
                .or_insert(InventoryState {
                    sku: order.sku.clone(),
                    qty: 0,
                    unit_cost: order.unit_price,
                    shelf_life_days: 120,
                });
            inv.qty += order.qty;
        }

        // Simulate sales per machine
        for machine in self.state.machines.values_mut() {
            let season_idx = self.turn % self.scenario.demand_model.seasonality.len();
            let season_mult = self.scenario.demand_model.seasonality[season_idx];

            for (sku, price) in machine.prices.iter() {
                let base_price = 2.0;
                let price_ratio = price / base_price;
                let price_effect =
                    1.0 + self.scenario.demand_model.price_elasticity * (price_ratio - 1.0);

                let base_demand = self.scenario.demand_model.base_daily_demand_per_machine;
                let trust_factor = 1.0 + 0.06 * (self.state.trust as f64 - 2.0);
                let uptime_factor = machine.uptime;

                let daily_demand = base_demand
                    * season_mult
                    * price_effect.max(0.1)
                    * trust_factor
                    * uptime_factor;
                let demand = (daily_demand * 7.0).round() as usize;

                let stock = machine.stock.entry(sku.clone()).or_insert(0);
                let sold = (*stock).min(demand);
                let _lost_sales = demand.saturating_sub(sold);

                // Track demand fulfillment for service level calculation
                self.total_demand += demand;
                self.fulfilled_demand += sold;
                self.total_turns_tracked += 1;
                if *stock == 0 {
                    self.stockout_turns += 1;
                }

                *stock = stock.saturating_sub(sold);

                let revenue = sold as f64 * price;
                self.state.cash += revenue;

                let cost = sold as f64
                    * self
                        .state
                        .inventory
                        .get(sku)
                        .map(|i| i.unit_cost)
                        .unwrap_or(0.5);
                self.state.cash -= cost;
            }

            if machine.maintenance_pending {
                machine.uptime = (machine.uptime + 0.02).min(1.0);
                machine.maintenance_pending = false;
                self.state.cash -= 50.0;
            }
        }

        // Process events
        for event in &self.scenario.events {
            if event.turn == self.turn {
                match event.event_type.as_str() {
                    "supplier_delay_risk" => {
                        let penalty = match event.severity.as_str() {
                            "high" => 0.7,
                            "medium" => 0.85,
                            _ => 0.9,
                        };
                        for supplier in &self.scenario.suppliers {
                            if self.rng.next() % 100 < 30 {
                                if let Some(contract) = self
                                    .state
                                    .contracts
                                    .get_mut(&format!("{}_{}", supplier.supplier_id, "water"))
                                {
                                    contract.lead_time_days =
                                        (contract.lead_time_days as f64 / penalty) as usize;
                                }
                            }
                        }
                    }
                    "local_event_spike" => {
                        let boost = match event.severity.as_str() {
                            "high" => 1.5,
                            "medium" => 1.25,
                            _ => 1.15,
                        };
                        // NOTE: modifying scenario demands is not ideal; keeping for compatibility
                        let season_idx = self.turn % self.scenario.demand_model.seasonality.len();
                        if season_idx < self.scenario.demand_model.seasonality.len() {
                            self.scenario.demand_model.seasonality[season_idx] *= boost;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Trust decay/regeneration
        self.state.trust = (self.state.trust + 1).min(4);

        if self.state.cash < self.scenario.constraints.min_cash_floor {
            self.violation_count += 1;
        }
    }

    pub fn is_complete(&self) -> bool {
        self.turn >= self.scenario.horizon_turns
    }

    pub fn compute_final_score(&self) -> FinalScore {
        let raw_profit = self.state.cash - self.scenario.initial_state.cash;

        // Calculate service level based on actual demand fulfillment over time
        // This is more accurate than checking final stock levels
        let service_level = if self.total_demand > 0 {
            self.fulfilled_demand as f64 / self.total_demand as f64
        } else {
            // Fallback: if no demand was generated yet, use stockout rate from turns
            if self.total_turns_tracked > 0 {
                1.0 - (self.stockout_turns as f64 / self.total_turns_tracked as f64)
            } else {
                1.0 // Default to perfect service if no data
            }
        };

        let solvency_score = if self.state.cash >= self.scenario.constraints.min_cash_floor {
            1.0
        } else {
            (self.state.cash / self.scenario.constraints.min_cash_floor).max(0.0)
        };

        let max_violations = self.scenario.horizon_turns / 4;
        let compliance_score =
            1.0 - (self.violation_count as f64 / max_violations.max(1) as f64).min(1.0);

        // Normalize profit against optimistic target (50k profit)
        let norm_profit = (raw_profit / 50000.0).clamp(0.0, 1.0);

        let weighted = self.scenario.scoring.profit_weight * norm_profit
            + self.scenario.scoring.service_level_weight * service_level
            + self.scenario.scoring.solvency_weight * solvency_score
            + self.scenario.scoring.compliance_weight * compliance_score;

        FinalScore {
            raw_profit,
            normalized_profit: norm_profit,
            service_level,
            solvency_score,
            compliance_score,
            weighted_score: weighted,
            stockout_rate: 1.0 - service_level,
            final_cash: self.state.cash,
            min_cash: self.scenario.constraints.min_cash_floor,
            bankrupt_turn: None,
            total_emails_sent: self.email_log.len(),
        }
    }
}

// ---------------------------------------------------------------------------
// Scenario loading
// ---------------------------------------------------------------------------

pub fn load_scenario(scenario_json: &str) -> Result<Scenario, String> {
    serde_json::from_str(scenario_json).map_err(|e| format!("failed to parse scenario: {}", e))
}
