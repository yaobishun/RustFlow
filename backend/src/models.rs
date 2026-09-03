use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub hourly_rate_cents: i64,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Deserialize)]
pub struct CreateUser {
    pub username: String,
    pub password: String,
    pub display_name: String,
    pub role: String,
    pub hourly_rate_cents: i64,
}
#[derive(Debug, Deserialize)]
pub struct UpdateUser {
    pub display_name: Option<String>,
    pub hourly_rate_cents: Option<i64>,
    pub active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeam {
    pub name: String,
    #[serde(default)]
    pub description: String,
}
#[derive(Debug, Deserialize)]
pub struct AddTeamMember {
    pub user_id: i64,
    pub role: String,
    #[serde(default = "capacity")]
    pub weekly_capacity_hours: f64,
}
fn capacity() -> f64 {
    40.0
}

#[derive(Debug, Deserialize)]
pub struct CreateProject {
    pub team_id: i64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub manager_id: i64,
    pub start_date: String,
    pub end_date: String,
    pub budget_cents: i64,
}
#[derive(Debug, Deserialize)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub description: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub budget_cents: Option<i64>,
    pub status: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct AddProjectMember {
    pub user_id: i64,
    pub project_role: String,
}
#[derive(Debug, Deserialize)]
pub struct CreateMilestone {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTask {
    pub milestone_id: Option<i64>,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub acceptance_criteria: String,
    pub assignee_id: Option<i64>,
    #[serde(default)]
    pub participant_ids: Vec<i64>,
    #[serde(default = "priority")]
    pub priority: String,
    pub planned_start: String,
    pub planned_end: String,
    #[serde(default)]
    pub estimated_hours: f64,
}
fn priority() -> String {
    "medium".into()
}
#[derive(Debug, Deserialize)]
pub struct UpdateTask {
    pub milestone_id: Option<i64>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub assignee_id: Option<i64>,
    pub participant_ids: Option<Vec<i64>>,
    pub priority: Option<String>,
    pub planned_start: Option<String>,
    pub planned_end: Option<String>,
    pub estimated_hours: Option<f64>,
    pub progress: Option<i64>,
}
#[derive(Debug, Deserialize)]
pub struct TransitionTask {
    pub to_status: String,
}
#[derive(Debug, Deserialize)]
pub struct CreateComment {
    pub content: String,
}
#[derive(Debug, Deserialize)]
pub struct ReviewTask {
    pub result: String,
    #[serde(default)]
    pub comment: String,
}
#[derive(Debug, Deserialize)]
pub struct AddDependency {
    pub depends_on_task_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorklog {
    pub task_id: i64,
    pub work_date: String,
    pub hours: f64,
    pub content: String,
}
#[derive(Debug, Deserialize)]
pub struct CreateExpense {
    pub category: String,
    pub amount_cents: i64,
    pub occurred_on: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateDecision {
    pub project_id: i64,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "one")]
    pub analysis_years: i64,
    pub options: Vec<CreateDecisionOption>,
    pub metrics: Vec<CreateDecisionMetric>,
}
fn one() -> i64 {
    1
}
#[derive(Debug, Deserialize)]
pub struct CreateDecisionOption {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub initial_cost_cents: i64,
    pub development_cost_cents: i64,
    pub annual_operation_cost_cents: i64,
    pub risk_probability_bps: i64,
    pub risk_loss_cents: i64,
    pub expected_benefit_cents: i64,
    pub values: Vec<f64>,
}
#[derive(Debug, Deserialize)]
pub struct CreateDecisionMetric {
    pub name: String,
    pub weight_bps: i64,
    pub direction: String,
    #[serde(default)]
    pub unit: String,
}
#[derive(Debug, Deserialize)]
pub struct ConfirmDecision {
    pub option_id: i64,
    pub reason: String,
}
