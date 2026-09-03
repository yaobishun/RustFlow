use std::collections::{HashMap, HashSet, VecDeque};

use crate::error::{ApiError, ApiResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Todo,
    InProgress,
    Review,
    Testing,
    Done,
}

impl TaskStatus {
    pub fn parse(s: &str) -> ApiResult<Self> {
        match s {
            "todo" => Ok(Self::Todo),
            "in_progress" => Ok(Self::InProgress),
            "review" => Ok(Self::Review),
            "testing" => Ok(Self::Testing),
            "done" => Ok(Self::Done),
            _ => Err(ApiError::BadRequest("未知任务状态".into())),
        }
    }
}

pub fn valid_transition(from: TaskStatus, to: TaskStatus, role: &str) -> bool {
    use TaskStatus::*;
    match (from, to) {
        (Todo, InProgress) | (InProgress, Review) => {
            matches!(role, "admin" | "manager" | "developer")
        }
        (Review, Testing) | (Testing, Done) => matches!(role, "admin" | "manager" | "reviewer"),
        (Review, InProgress) | (Testing, InProgress) => {
            matches!(role, "admin" | "manager" | "reviewer")
        }
        _ => false,
    }
}

pub fn validate_review_input(result: &str, comment: &str) -> ApiResult<()> {
    if !["approved", "rejected"].contains(&result) {
        return Err(ApiError::BadRequest(
            "评审结果必须为approved或rejected".into(),
        ));
    }
    if result == "rejected" && comment.trim().is_empty() {
        return Err(ApiError::BadRequest("驳回评审时必须填写原因".into()));
    }
    Ok(())
}

pub fn would_create_cycle(edges: &[(i64, i64)], task_id: i64, depends_on: i64) -> bool {
    if task_id == depends_on {
        return true;
    }
    let mut outgoing: HashMap<i64, Vec<i64>> = HashMap::new();
    // edge (task, dependency): cycle exists if task is reachable by following dependencies from dependency
    for &(task, dep) in edges {
        outgoing.entry(task).or_default().push(dep);
    }
    outgoing.entry(task_id).or_default().push(depends_on);
    let mut stack = vec![depends_on];
    let mut seen = HashSet::new();
    while let Some(node) = stack.pop() {
        if node == task_id {
            return true;
        }
        if seen.insert(node)
            && let Some(next) = outgoing.get(&node)
        {
            stack.extend(next);
        }
    }
    false
}

pub fn downstream(edges: &[(i64, i64)], source: i64) -> Vec<i64> {
    let mut reverse: HashMap<i64, Vec<i64>> = HashMap::new();
    for &(task, dep) in edges {
        reverse.entry(dep).or_default().push(task);
    }
    let mut queue = VecDeque::from([source]);
    let mut seen = HashSet::new();
    while let Some(node) = queue.pop_front() {
        if let Some(next) = reverse.get(&node) {
            for &id in next {
                if seen.insert(id) {
                    queue.push_back(id);
                }
            }
        }
    }
    let mut result: Vec<_> = seen.into_iter().collect();
    result.sort_unstable();
    result
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EvmMetrics {
    pub pv_cents: i64,
    pub ev_cents: i64,
    pub ac_cents: i64,
    pub bac_cents: i64,
    pub spi: Option<f64>,
    pub cpi: Option<f64>,
    pub eac_cents: Option<i64>,
    pub health: String,
}

pub fn calculate_evm(pv: i64, ev: i64, ac: i64, bac: i64) -> EvmMetrics {
    let spi = (pv > 0).then(|| ev as f64 / pv as f64);
    let cpi = (ac > 0).then(|| ev as f64 / ac as f64);
    let eac = cpi
        .filter(|v| *v > 0.0)
        .map(|v| (bac as f64 / v).round() as i64);
    let progress_bad = spi.is_some_and(|v| v < 1.0);
    let cost_bad = cpi.is_some_and(|v| v < 1.0);
    let health = match (progress_bad, cost_bad) {
        (true, true) => "critical",
        (true, false) => "schedule_risk",
        (false, true) => "cost_risk",
        _ => "healthy",
    }
    .to_string();
    EvmMetrics {
        pv_cents: pv,
        ev_cents: ev,
        ac_cents: ac,
        bac_cents: bac,
        spi,
        cpi,
        eac_cents: eac,
        health,
    }
}

pub fn expected_risk_loss(loss_cents: i64, probability_bps: i64) -> i64 {
    ((loss_cents as i128 * probability_bps as i128) / 10_000) as i64
}

pub fn tco(initial: i64, development: i64, annual_ops: i64, years: i64, risk: i64) -> i64 {
    initial
        .saturating_add(development)
        .saturating_add(annual_ops.saturating_mul(years))
        .saturating_add(risk)
}

pub fn roi_bps(benefit: i64, total_cost: i64) -> ApiResult<i64> {
    if total_cost <= 0 {
        return Err(ApiError::BadRequest("TCO必须大于0，无法计算ROI".into()));
    }
    Ok((((benefit - total_cost) as i128 * 10_000) / total_cost as i128) as i64)
}

pub fn normalize(values: &[f64], higher_is_better: bool) -> Vec<f64> {
    if values.is_empty() {
        return vec![];
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if (max - min).abs() < f64::EPSILON {
        return vec![100.0; values.len()];
    }
    values
        .iter()
        .map(|&v| {
            if higher_is_better {
                (v - min) / (max - min) * 100.0
            } else {
                (max - v) / (max - min) * 100.0
            }
        })
        .collect()
}

/// TCO 与 ROI 是后端经济模型的派生指标，不能信任前端提交的占位值。
pub fn resolve_decision_metric(
    metric_name: &str,
    supplied_value: f64,
    tco_cents: i64,
    roi_bps: i64,
) -> f64 {
    let name = metric_name.trim();
    if is_tco_metric(name) {
        tco_cents as f64 / 100.0
    } else if is_roi_metric(name) {
        roi_bps as f64 / 100.0
    } else {
        supplied_value
    }
}

pub fn is_tco_metric(name: &str) -> bool {
    name.trim().eq_ignore_ascii_case("tco") || name.contains("总拥有成本")
}

pub fn is_roi_metric(name: &str) -> bool {
    name.trim().eq_ignore_ascii_case("roi")
        || name.contains("投资回报率")
        || name.contains("投资回报")
}

pub fn can_manage_project(project_role: &str) -> bool {
    project_role == "admin" || project_role == "manager"
}

pub fn can_contribute_task(project_role: &str, assigned_or_participant: bool) -> bool {
    can_manage_project(project_role) || (project_role == "developer" && assigned_or_participant)
}

pub fn can_review_task(project_role: &str) -> bool {
    can_manage_project(project_role) || project_role == "reviewer"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_machine_enforces_roles() {
        assert!(valid_transition(
            TaskStatus::Todo,
            TaskStatus::InProgress,
            "developer"
        ));
        assert!(!valid_transition(
            TaskStatus::Review,
            TaskStatus::Testing,
            "developer"
        ));
        assert!(valid_transition(
            TaskStatus::Review,
            TaskStatus::Testing,
            "reviewer"
        ));
    }
    #[test]
    fn rejection_requires_a_reason() {
        assert!(validate_review_input("rejected", "").is_err());
        assert!(validate_review_input("rejected", "验收标准未满足").is_ok());
        assert!(validate_review_input("approved", "").is_ok());
    }
    #[test]
    fn graph_cycle_and_downstream() {
        let edges = vec![(2, 1), (3, 2), (4, 1)];
        assert!(would_create_cycle(&edges, 1, 3));
        assert_eq!(downstream(&edges, 1), vec![2, 3, 4]);
    }
    #[test]
    fn economic_math_is_integer_safe() {
        let risk = expected_risk_loss(1_000_000, 2_000);
        let total = tco(2_000_000, 600_000, 800_000, 3, risk);
        assert_eq!(total, 5_200_000);
        assert_eq!(roi_bps(7_000_000, total).unwrap(), 3461);
    }
    #[test]
    fn normalization_handles_cost_and_equal_values() {
        assert_eq!(
            normalize(&[10.0, 20.0, 30.0], false),
            vec![100.0, 50.0, 0.0]
        );
        assert_eq!(normalize(&[5.0, 5.0], true), vec![100.0, 100.0]);
    }
    #[test]
    fn evm_reports_dual_risk() {
        let m = calculate_evm(100, 70, 100, 200);
        assert_eq!(m.health, "critical");
        assert_eq!(m.eac_cents, Some(286));
    }

    #[test]
    fn evm_below_one_is_not_healthy() {
        let m = calculate_evm(100, 95, 90, 200);
        assert_eq!(m.health, "schedule_risk");
    }

    #[test]
    fn economic_decision_metrics_ignore_client_placeholders() {
        assert_eq!(
            resolve_decision_metric("tCo", 0.0, 4_280_000, 6_355),
            42_800.0
        );
        assert_eq!(
            resolve_decision_metric(" ROI ", -99.0, 4_280_000, 6_355),
            63.55
        );
        assert_eq!(
            resolve_decision_metric("安全性", 88.0, 4_280_000, 6_355),
            88.0
        );
        assert_eq!(
            resolve_decision_metric("总拥有成本", 1.0, 4_280_000, 6_355),
            42_800.0
        );
        assert_eq!(
            resolve_decision_metric("投资回报率", 1.0, 4_280_000, 6_355),
            63.55
        );
    }

    #[test]
    fn project_role_policy_is_explicit() {
        assert!(can_manage_project("manager"));
        assert!(!can_manage_project("developer"));
        assert!(can_contribute_task("developer", true));
        assert!(!can_contribute_task("developer", false));
        assert!(can_review_task("reviewer"));
        assert!(!can_review_task("developer"));
    }
}
