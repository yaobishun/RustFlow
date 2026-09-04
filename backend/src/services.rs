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

/// 等值（无区分度）指标归一化后的中性分。避免"所有方案数值相同却拿满分"虚增优势。
const NEUTRAL_SCORE: f64 = 50.0;
/// 样本量达到该阈值时才启用分位缩尾，避免小样本下分位估计失真。
const WINSORIZE_MIN_N: usize = 10;
const WINSORIZE_LOWER: f64 = 0.05;
const WINSORIZE_UPPER: f64 = 0.95;

pub fn normalize(values: &[f64], higher_is_better: bool) -> Vec<f64> {
    if values.is_empty() {
        return vec![];
    }
    if values.len() == 1 {
        // 单方案没有比较基准，给中性分。
        return vec![NEUTRAL_SCORE];
    }
    let mut v = values.to_vec();
    if values.len() >= WINSORIZE_MIN_N {
        // 抗离群：先对两端做分位缩尾，避免单一极端值压扁其余分差。
        winsorize(&mut v, WINSORIZE_LOWER, WINSORIZE_UPPER);
    }
    let min = v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if (max - min).abs() < f64::EPSILON {
        // 全部相等（或缩尾后相等）：无区分度，给中性分。
        return vec![NEUTRAL_SCORE; values.len()];
    }
    v.iter()
        .map(|&x| {
            if higher_is_better {
                (x - min) / (max - min) * 100.0
            } else {
                (max - x) / (max - min) * 100.0
            }
        })
        .collect()
}

/// 判断一组值是否无区分度（全部相等，或仅一个值）。
pub fn is_uniform(values: &[f64]) -> bool {
    if values.len() < 2 {
        return true;
    }
    let first = values[0];
    values.iter().all(|&x| (x - first).abs() < f64::EPSILON)
}

/// 对 values 两端做分位缩尾（winsorization）：落在 lower/upper 分位之外的值
/// 截断到分位边界，用线性插值分位数计算边界。
fn winsorize(values: &mut [f64], lower: f64, upper: f64) {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let lo = quantile(&sorted, lower);
    let hi = quantile(&sorted, upper);
    for v in values.iter_mut() {
        if *v < lo {
            *v = lo;
        }
        if *v > hi {
            *v = hi;
        }
    }
}

fn quantile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n <= 1 {
        return sorted[0];
    }
    let pos = p * (n as f64 - 1.0);
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    let frac = pos - lo as f64;
    sorted[lo] + (sorted[hi] - sorted[lo]) * frac
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

#[derive(Debug, Clone)]
pub struct AllocationMember {
    pub id: i64,
    pub name: String,
    pub role: String,
    pub weekly_capacity: f64,
    pub hourly_rate: f64,
}

#[derive(Debug, Clone)]
pub struct AllocationTask {
    pub id: i64,
    pub title: String,
    pub priority: String,
    pub assignee_id: Option<i64>,
    pub remaining_hours: f64,
    pub start_week: usize,
    pub end_week: usize,
    pub downstream_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AllocationAssignment {
    pub task_id: i64,
    pub task_title: String,
    pub from_user_id: Option<i64>,
    pub to_user_id: i64,
    pub to_user_name: String,
    pub score: f64,
    pub projected_peak_load: f64,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AllocationMemberImpact {
    pub user_id: i64,
    pub user_name: String,
    pub before_peak_load: f64,
    pub after_peak_load: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AllocationPlan {
    pub assignments: Vec<AllocationAssignment>,
    pub member_impacts: Vec<AllocationMemberImpact>,
    pub overload_before: usize,
    pub overload_after: usize,
}

fn distribute(loads: &mut [f64], task: &AllocationTask, sign: f64) {
    if loads.is_empty() {
        return;
    }
    let start = task.start_week.min(loads.len() - 1);
    let end = task.end_week.max(start).min(loads.len() - 1);
    let per_week = task.remaining_hours.max(0.0) / (end - start + 1) as f64;
    for value in &mut loads[start..=end] {
        *value += per_week * sign;
    }
}

fn peak_rate(loads: &[f64], capacity: f64) -> f64 {
    if capacity <= 0.0 {
        return 0.0;
    }
    loads.iter().copied().fold(0.0, f64::max) / capacity * 100.0
}

/// Greedy, deterministic workload allocation. Locked work is counted first; target tasks are
/// ordered by business urgency, then placed where their marginal overload is smallest.
pub fn allocate_workload(
    members: &[AllocationMember],
    locked_tasks: &[AllocationTask],
    target_tasks: &[AllocationTask],
    weeks: usize,
) -> AllocationPlan {
    let weeks = weeks.max(1);
    let mut loads: HashMap<i64, Vec<f64>> =
        members.iter().map(|m| (m.id, vec![0.0; weeks])).collect();
    for task in locked_tasks {
        if let Some(member_loads) = task.assignee_id.and_then(|id| loads.get_mut(&id)) {
            distribute(member_loads, task, 1.0);
        }
    }
    let mut before = loads.clone();
    for task in target_tasks {
        if let Some(member_loads) = task.assignee_id.and_then(|id| before.get_mut(&id)) {
            distribute(member_loads, task, 1.0);
        }
    }
    let mut tasks = target_tasks.to_vec();
    let priority = |p: &str| match p {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        _ => 1,
    };
    tasks.sort_by(|a, b| {
        priority(&b.priority)
            .cmp(&priority(&a.priority))
            .then(a.end_week.cmp(&b.end_week))
            .then(b.downstream_count.cmp(&a.downstream_count))
            .then_with(|| b.remaining_hours.total_cmp(&a.remaining_hours))
            .then(a.id.cmp(&b.id))
    });

    let min_rate = members
        .iter()
        .map(|m| m.hourly_rate)
        .fold(f64::INFINITY, f64::min);
    let max_rate = members
        .iter()
        .map(|m| m.hourly_rate)
        .fold(f64::NEG_INFINITY, f64::max);
    let mut assignments = Vec::new();
    for task in tasks {
        let normalized_title = task.title.to_ascii_lowercase();
        let review_work = ["测试", "评审", "验收", "test", "review", "qa"]
            .iter()
            .any(|keyword| normalized_title.contains(keyword));
        let preferred = members
            .iter()
            .filter(|member| {
                if review_work {
                    member.role == "reviewer"
                } else {
                    matches!(member.role.as_str(), "developer" | "manager" | "admin")
                }
            })
            .collect::<Vec<_>>();
        let candidates = if preferred.is_empty() {
            members.iter().collect::<Vec<_>>()
        } else {
            preferred
        };
        let mut best: Option<(f64, f64, &AllocationMember, Vec<f64>)> = None;
        for member in candidates {
            let mut projected = loads
                .get(&member.id)
                .cloned()
                .unwrap_or_else(|| vec![0.0; weeks]);
            distribute(&mut projected, &task, 1.0);
            let peak = peak_rate(&projected, member.weekly_capacity);
            let start = task.start_week.min(weeks - 1);
            let end = task.end_week.max(start).min(weeks - 1);
            let avg_rate = projected[start..=end].iter().sum::<f64>()
                / (end - start + 1) as f64
                / member.weekly_capacity.max(0.1)
                * 100.0;
            let balance_score = (100.0 - (avg_rate - 85.0).abs()).clamp(0.0, 100.0);
            let availability_score = if peak <= 100.0 { 100.0 } else { 0.0 };
            let role_score = match (review_work, member.role.as_str()) {
                (true, "reviewer") | (false, "developer") => 100.0,
                (false, "manager" | "admin") => 80.0,
                _ => 60.0,
            };
            let cost_score = if (max_rate - min_rate).abs() < f64::EPSILON {
                50.0
            } else {
                (max_rate - member.hourly_rate) / (max_rate - min_rate) * 100.0
            };
            let continuity = if task.assignee_id == Some(member.id) {
                5.0
            } else {
                0.0
            };
            let overload_penalty = (peak - 100.0).max(0.0) * 3.0;
            let score = 0.55 * balance_score
                + 0.25 * availability_score
                + 0.10 * role_score
                + 0.10 * cost_score
                + continuity
                - overload_penalty;
            let replace = best
                .as_ref()
                .is_none_or(|(best_score, best_peak, best_member, _)| {
                    score > *best_score
                        || ((score - *best_score).abs() < 1e-9
                            && (peak < *best_peak
                                || ((peak - *best_peak).abs() < 1e-9
                                    && member.id < best_member.id)))
                });
            if replace {
                best = Some((score, peak, member, projected));
            }
        }
        if let Some((score, peak, member, projected)) = best {
            loads.insert(member.id, projected);
            if task.assignee_id == Some(member.id) {
                continue;
            }
            let reason = if peak <= 85.0 {
                format!("分配后峰值负载{peak:.0}%，容量充足")
            } else if peak <= 100.0 {
                format!("分配后峰值负载{peak:.0}%，仍在可用容量内")
            } else {
                format!("团队容量不足；该成员分配后峰值负载最低（{peak:.0}%）")
            };
            assignments.push(AllocationAssignment {
                task_id: task.id,
                task_title: task.title,
                from_user_id: task.assignee_id,
                to_user_id: member.id,
                to_user_name: member.name.clone(),
                score: (score * 10.0).round() / 10.0,
                projected_peak_load: (peak * 10.0).round() / 10.0,
                reason,
            });
        }
    }
    let impacts = members
        .iter()
        .map(|m| AllocationMemberImpact {
            user_id: m.id,
            user_name: m.name.clone(),
            before_peak_load: (peak_rate(&before[&m.id], m.weekly_capacity) * 10.0).round() / 10.0,
            after_peak_load: (peak_rate(&loads[&m.id], m.weekly_capacity) * 10.0).round() / 10.0,
        })
        .collect::<Vec<_>>();
    AllocationPlan {
        overload_before: impacts
            .iter()
            .filter(|x| x.before_peak_load > 100.0)
            .count(),
        overload_after: impacts.iter().filter(|x| x.after_peak_load > 100.0).count(),
        assignments,
        member_impacts: impacts,
    }
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
        // 等值（无区分度）与单方案统一给中性分，而不是满分。
        assert_eq!(normalize(&[5.0, 5.0], true), vec![50.0, 50.0]);
        assert_eq!(normalize(&[5.0, 5.0], false), vec![50.0, 50.0]);
        assert_eq!(normalize(&[7.0], true), vec![50.0]);
    }

    #[test]
    fn normalization_winsorizes_outliers_for_large_samples() {
        let vals = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 1000.0];
        let got = normalize(&vals, true);
        // 极端值 1000 被缩尾截断；其余小值之间的分差得以保留、放大。
        assert_eq!(got[9], 100.0);
        assert!(got[8] > 1.0);
        assert!(got[8] < 20.0);
    }

    #[test]
    fn uniform_detection_covers_equal_and_single() {
        assert!(is_uniform(&[4.0, 4.0, 4.0]));
        assert!(!is_uniform(&[4.0, 5.0, 4.0]));
        assert!(is_uniform(&[4.0]));
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
    fn workload_allocation_balances_weekly_capacity_and_is_deterministic() {
        let members = vec![
            AllocationMember {
                id: 1,
                name: "甲".into(),
                role: "developer".into(),
                weekly_capacity: 40.0,
                hourly_rate: 80.0,
            },
            AllocationMember {
                id: 2,
                name: "乙".into(),
                role: "developer".into(),
                weekly_capacity: 40.0,
                hourly_rate: 80.0,
            },
        ];
        let locked = vec![AllocationTask {
            id: 1,
            title: "锁定".into(),
            priority: "high".into(),
            assignee_id: Some(1),
            remaining_hours: 32.0,
            start_week: 0,
            end_week: 0,
            downstream_count: 0,
        }];
        let targets = vec![AllocationTask {
            id: 2,
            title: "待分配".into(),
            priority: "critical".into(),
            assignee_id: None,
            remaining_hours: 24.0,
            start_week: 0,
            end_week: 0,
            downstream_count: 2,
        }];
        let plan = allocate_workload(&members, &locked, &targets, 1);
        assert_eq!(plan.assignments[0].to_user_id, 2);
        assert_eq!(plan.overload_after, 0);
    }

    #[test]
    fn workload_allocation_preserves_source_in_preview() {
        let members = vec![
            AllocationMember {
                id: 1,
                name: "甲".into(),
                role: "developer".into(),
                weekly_capacity: 20.0,
                hourly_rate: 60.0,
            },
            AllocationMember {
                id: 2,
                name: "乙".into(),
                role: "developer".into(),
                weekly_capacity: 40.0,
                hourly_rate: 60.0,
            },
        ];
        let target = AllocationTask {
            id: 9,
            title: "任务".into(),
            priority: "medium".into(),
            assignee_id: Some(1),
            remaining_hours: 30.0,
            start_week: 0,
            end_week: 0,
            downstream_count: 0,
        };
        let plan = allocate_workload(&members, &[], &[target], 1);
        assert_eq!(plan.assignments[0].from_user_id, Some(1));
        assert_eq!(plan.assignments[0].to_user_id, 2);
    }

    #[test]
    fn workload_allocation_respects_task_role_preference() {
        let members = vec![
            AllocationMember {
                id: 1,
                name: "开发".into(),
                role: "developer".into(),
                weekly_capacity: 40.0,
                hourly_rate: 60.0,
            },
            AllocationMember {
                id: 2,
                name: "测试".into(),
                role: "reviewer".into(),
                weekly_capacity: 40.0,
                hourly_rate: 60.0,
            },
        ];
        let target = AllocationTask {
            id: 10,
            title: "完成回归测试".into(),
            priority: "high".into(),
            assignee_id: None,
            remaining_hours: 8.0,
            start_week: 0,
            end_week: 0,
            downstream_count: 0,
        };
        let plan = allocate_workload(&members, &[], &[target], 1);
        assert_eq!(plan.assignments[0].to_user_id, 2);
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
