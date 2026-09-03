use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderValue, Method, StatusCode},
    routing::{delete, get, post, put},
};
use chrono::{NaiveDate, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Row, SqlitePool};
use std::collections::BTreeMap;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{
    AppState,
    auth::{self, Actor},
    error::{ApiError, ApiResult},
    models::*,
    services::{self, TaskStatus},
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { Json(json!({"status":"ok"})) }))
        .route(
            "/api/health",
            get(|| async { Json(json!({"status":"ok"})) }),
        )
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(me))
        .route("/api/auth/logout", post(logout))
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/{id}", put(update_user))
        .route("/api/teams", get(list_teams).post(create_team))
        .route(
            "/api/teams/{id}/members",
            get(team_members).post(add_team_member),
        )
        .route("/api/projects", get(list_projects).post(create_project))
        .route("/api/projects/{id}", get(get_project).put(update_project))
        .route(
            "/api/projects/{id}/members",
            get(project_members).post(add_project_member),
        )
        .route(
            "/api/projects/{id}/milestones",
            get(list_milestones).post(create_milestone),
        )
        .route(
            "/api/projects/{id}/tasks",
            get(list_tasks).post(create_task),
        )
        .route(
            "/api/projects/{id}/worklogs",
            get(project_worklogs).post(create_worklog_for_project),
        )
        .route(
            "/api/projects/{id}/expenses",
            get(project_expenses).post(create_expense),
        )
        .route("/api/projects/{id}/finance/summary", get(finance_summary))
        .route("/api/projects/{id}/metrics", get(project_metrics))
        .route("/api/projects/{id}/risks", get(project_risks))
        .route("/api/finance/summary", get(global_finance_summary))
        .route(
            "/api/worklogs",
            get(global_worklogs).post(global_create_worklog),
        )
        .route(
            "/api/expenses",
            get(global_expenses).post(global_create_expense),
        )
        .route("/api/risks", get(global_risks))
        .route("/api/risks/scan", post(scan_risks))
        .route("/api/tasks/{id}", get(get_task).put(update_task))
        .route(
            "/api/tasks/{id}/comments",
            get(task_comments).post(add_comment),
        )
        .route("/api/tasks/{id}/transition", post(transition_task))
        .route("/api/tasks/{id}/review", post(review_task))
        .route(
            "/api/tasks/{id}/dependencies",
            get(task_dependencies).post(add_dependency),
        )
        .route(
            "/api/tasks/{id}/dependencies/{dependency_id}",
            delete(remove_dependency),
        )
        .route("/api/tasks/{id}/downstream", get(task_downstream))
        .route("/api/decisions", get(list_decisions).post(create_decision))
        .route("/api/decisions/{id}", get(get_decision))
        .route("/api/decisions/{id}/evaluate", post(evaluate_decision))
        .route("/api/decisions/{id}/confirm", post(confirm_decision))
        .route("/api/dashboard", get(dashboard))
        .route("/api/activity-logs", get(activity_logs))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(HeaderValue::from_static("*"))
                .allow_headers([
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::CONTENT_TYPE,
                ])
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE]),
        )
}

async fn log(
    db: &SqlitePool,
    actor: i64,
    entity: &str,
    id: Option<i64>,
    action: &str,
    detail: &str,
) {
    let _ = sqlx::query(
        "INSERT INTO activity_logs(actor_id,entity_type,entity_id,action,detail) VALUES(?,?,?,?,?)",
    )
    .bind(actor)
    .bind(entity)
    .bind(id)
    .bind(action)
    .bind(detail)
    .execute(db)
    .await;
}
async fn require_project_member(db: &SqlitePool, actor: &Actor, project_id: i64) -> ApiResult<()> {
    project_role(db, actor, project_id).await.map(|_| ())
}
async fn project_role(db: &SqlitePool, actor: &Actor, project_id: i64) -> ApiResult<String> {
    if actor.role == "admin" {
        return Ok("admin".into());
    }
    sqlx::query_scalar("SELECT project_role FROM project_members WHERE project_id=? AND user_id=?")
        .bind(project_id)
        .bind(actor.id)
        .fetch_optional(db)
        .await?
        .ok_or(ApiError::Forbidden)
}
async fn require_active_project(db: &SqlitePool, project_id: i64) -> ApiResult<()> {
    let status: String = sqlx::query_scalar("SELECT status FROM projects WHERE id=?")
        .bind(project_id)
        .fetch_optional(db)
        .await?
        .ok_or(ApiError::NotFound)?;
    if status == "archived" {
        Err(ApiError::BadRequest("归档项目为只读状态".into()))
    } else {
        Ok(())
    }
}
async fn require_project_manager(db: &SqlitePool, actor: &Actor, project_id: i64) -> ApiResult<()> {
    require_active_project(db, project_id).await?;
    let role = project_role(db, actor, project_id).await?;
    if services::can_manage_project(&role) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}
async fn task_project_id(db: &SqlitePool, task_id: i64) -> ApiResult<i64> {
    sqlx::query_scalar("SELECT project_id FROM tasks WHERE id=?")
        .bind(task_id)
        .fetch_optional(db)
        .await?
        .ok_or(ApiError::NotFound)
}
async fn validate_task_relations(
    db: &SqlitePool,
    project_id: i64,
    milestone_id: Option<i64>,
    assignee_id: Option<i64>,
    participant_ids: &[i64],
) -> ApiResult<()> {
    if let Some(mid) = milestone_id {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM milestones WHERE id=? AND project_id=?")
                .bind(mid)
                .bind(project_id)
                .fetch_one(db)
                .await?;
        if count == 0 {
            return Err(ApiError::BadRequest("里程碑不属于该项目".into()));
        }
    }
    let mut users = participant_ids.to_vec();
    if let Some(uid) = assignee_id {
        users.push(uid)
    }
    users.sort_unstable();
    users.dedup();
    for uid in users {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM project_members WHERE project_id=? AND user_id=?",
        )
        .bind(project_id)
        .bind(uid)
        .fetch_one(db)
        .await?;
        if count == 0 {
            return Err(ApiError::BadRequest(format!("用户{uid}不是项目成员")));
        }
    }
    Ok(())
}
async fn require_task_contributor(db: &SqlitePool, actor: &Actor, task_id: i64) -> ApiResult<i64> {
    let project_id = task_project_id(db, task_id).await?;
    require_active_project(db, project_id).await?;
    let role = project_role(db, actor, project_id).await?;
    let assigned_or_participant: i64 = sqlx::query_scalar("SELECT CASE WHEN assignee_id=? OR EXISTS(SELECT 1 FROM task_participants WHERE task_id=tasks.id AND user_id=?) THEN 1 ELSE 0 END FROM tasks WHERE id=?")
        .bind(actor.id).bind(actor.id).bind(task_id).fetch_one(db).await?;
    if services::can_contribute_task(&role, assigned_or_participant > 0) {
        Ok(project_id)
    } else {
        Err(ApiError::Forbidden)
    }
}
async fn require_task_reviewer(db: &SqlitePool, actor: &Actor, task_id: i64) -> ApiResult<i64> {
    let project_id = task_project_id(db, task_id).await?;
    require_active_project(db, project_id).await?;
    let role = project_role(db, actor, project_id).await?;
    if services::can_review_task(&role) {
        Ok(project_id)
    } else {
        Err(ApiError::Forbidden)
    }
}
fn nonempty(value: &str, name: &str) -> ApiResult<()> {
    if value.trim().is_empty() {
        Err(ApiError::BadRequest(format!("{name}不能为空")))
    } else {
        Ok(())
    }
}
fn valid_date(s: &str) -> ApiResult<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| ApiError::BadRequest(format!("无效日期：{s}，应为YYYY-MM-DD")))
}
fn cents_to_yuan(c: i64) -> f64 {
    c as f64 / 100.0
}
fn priority_in(s: &str) -> &str {
    if s == "urgent" { "critical" } else { s }
}
fn metric_key(name: &str) -> String {
    let normalized = name.trim().to_ascii_lowercase();
    if normalized == "tco" || name.contains("总拥有成本") {
        "tco".into()
    } else if normalized == "roi" || name.contains("投资回报") {
        "roi".into()
    } else if name.contains("可行") || normalized.contains("feasibility") {
        "feasibility".into()
    } else if name.contains("周期") || normalized.contains("duration") {
        "duration".into()
    } else if name.contains("安全") || normalized.contains("security") {
        "security".into()
    } else if name.contains("扩展") || normalized.contains("scalability") {
        "scalability".into()
    } else {
        normalized.replace(' ', "_")
    }
}

async fn login(
    State(s): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Json<LoginResponse>> {
    let row=sqlx::query("SELECT id,username,password_hash,display_name,role,hourly_rate_cents,active FROM users WHERE username=?")
        .bind(&req.username).fetch_optional(&s.db).await?.ok_or(ApiError::Unauthorized)?;
    let active: bool = row.get("active");
    if !active || !auth::verify_password(&req.password, row.get("password_hash")) {
        return Err(ApiError::Unauthorized);
    }
    let user = User {
        id: row.get("id"),
        username: row.get("username"),
        display_name: row.get("display_name"),
        role: row.get("role"),
        hourly_rate_cents: row.get("hourly_rate_cents"),
        active,
    };
    let token = auth::token(user.id, &user.username, &user.role, &s.jwt_secret)?;
    log(&s.db, user.id, "auth", None, "login", "").await;
    Ok(Json(LoginResponse { token, user }))
}
async fn me(State(s): State<AppState>, actor: Actor) -> ApiResult<Json<Value>> {
    let r = sqlx::query(
        "SELECT id,username,display_name,role,hourly_rate_cents,active FROM users WHERE id=?",
    )
    .bind(actor.id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(ApiError::Unauthorized)?;
    Ok(Json(user_json(&r)))
}
async fn logout(State(s): State<AppState>, actor: Actor) -> ApiResult<Json<Value>> {
    // JWT保持无服务端会话；前端删除令牌即完成退出，这里只记录审计日志。
    log(&s.db, actor.id, "auth", None, "logout", "").await;
    Ok(Json(json!({"ok":true})))
}
fn user_json(r: &sqlx::sqlite::SqliteRow) -> Value {
    let name: String = r.get("display_name");
    json!({"id":r.get::<i64,_>("id"),"username":r.get::<String,_>("username"),"name":name,"display_name":name,"role":r.get::<String,_>("role"),"hourly_rate":cents_to_yuan(r.get("hourly_rate_cents")),"active":r.get::<bool,_>("active")})
}

async fn list_users(State(s): State<AppState>, _actor: Actor) -> ApiResult<Json<Value>> {
    let rows = sqlx::query(
        "SELECT id,username,display_name,role,hourly_rate_cents,active FROM users ORDER BY id",
    )
    .fetch_all(&s.db)
    .await?;
    Ok(Json(Value::Array(rows.iter().map(user_json).collect())))
}
async fn create_user(
    State(s): State<AppState>,
    actor: Actor,
    Json(req): Json<CreateUser>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth::require(&actor, &["admin"])?;
    nonempty(&req.username, "用户名")?;
    if !["admin", "manager", "developer", "reviewer"].contains(&req.role.as_str())
        || req.hourly_rate_cents < 0
    {
        return Err(ApiError::BadRequest("无效角色或时薪".into()));
    }
    use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
    use rand_core::OsRng;
    let hash = Argon2::default()
        .hash_password(req.password.as_bytes(), &SaltString::generate(&mut OsRng))
        .map_err(|_| ApiError::Internal)?
        .to_string();
    let out=sqlx::query("INSERT INTO users(username,password_hash,display_name,role,hourly_rate_cents) VALUES(?,?,?,?,?)").bind(&req.username).bind(hash).bind(&req.display_name).bind(&req.role).bind(req.hourly_rate_cents).execute(&s.db).await?;
    log(
        &s.db,
        actor.id,
        "user",
        Some(out.last_insert_rowid()),
        "create",
        &req.username,
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id":out.last_insert_rowid()})),
    ))
}
async fn update_user(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
    Json(req): Json<UpdateUser>,
) -> ApiResult<Json<Value>> {
    if actor.role != "admin" && actor.id != id {
        return Err(ApiError::Forbidden);
    }
    if actor.role != "admin" && (req.hourly_rate_cents.is_some() || req.active.is_some()) {
        return Err(ApiError::Forbidden);
    }
    sqlx::query("UPDATE users SET display_name=COALESCE(?,display_name),hourly_rate_cents=COALESCE(?,hourly_rate_cents),active=COALESCE(?,active) WHERE id=?")
        .bind(req.display_name).bind(req.hourly_rate_cents).bind(req.active).bind(id).execute(&s.db).await?;
    log(&s.db, actor.id, "user", Some(id), "update", "").await;
    Ok(Json(json!({"ok":true})))
}

async fn list_teams(State(s): State<AppState>, _actor: Actor) -> ApiResult<Json<Value>> {
    let rows=sqlx::query("SELECT t.id,t.name,t.description,t.owner_id,u.display_name owner_name,(SELECT COUNT(*) FROM team_members tm WHERE tm.team_id=t.id) member_count FROM teams t JOIN users u ON u.id=t.owner_id ORDER BY t.id").fetch_all(&s.db).await?;
    Ok(Json(Value::Array(rows.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"name":r.get::<String,_>("name"),"description":r.get::<String,_>("description"),"owner_id":r.get::<i64,_>("owner_id"),"owner_name":r.get::<String,_>("owner_name"),"member_count":r.get::<i64,_>("member_count")})).collect())))
}
async fn create_team(
    State(s): State<AppState>,
    actor: Actor,
    Json(req): Json<CreateTeam>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth::require(&actor, &["admin", "manager"])?;
    nonempty(&req.name, "团队名称")?;
    let mut tx = s.db.begin().await?;
    let out = sqlx::query("INSERT INTO teams(name,description,owner_id) VALUES(?,?,?)")
        .bind(&req.name)
        .bind(&req.description)
        .bind(actor.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO team_members(team_id,user_id,role) VALUES(?,?,'manager')")
        .bind(out.last_insert_rowid())
        .bind(actor.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    log(
        &s.db,
        actor.id,
        "team",
        Some(out.last_insert_rowid()),
        "create",
        &req.name,
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id":out.last_insert_rowid()})),
    ))
}
async fn team_members(
    State(s): State<AppState>,
    _actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let rows=sqlx::query("SELECT u.id,u.username,u.display_name,u.role,u.hourly_rate_cents,u.active,tm.role team_role,tm.weekly_capacity_hours FROM team_members tm JOIN users u ON u.id=tm.user_id WHERE tm.team_id=?").bind(id).fetch_all(&s.db).await?;
    Ok(Json(Value::Array(
        rows.iter()
            .map(|r| {
                let mut v = user_json(r);
                v["team_role"] = json!(r.get::<String, _>("team_role"));
                v["weekly_capacity_hours"] = json!(r.get::<f64, _>("weekly_capacity_hours"));
                v
            })
            .collect(),
    )))
}
async fn add_team_member(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
    Json(req): Json<AddTeamMember>,
) -> ApiResult<Json<Value>> {
    let team = sqlx::query("SELECT owner_id FROM teams WHERE id=?")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    if actor.role != "admin" {
        let is_manager: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM team_members WHERE team_id=? AND user_id=? AND role='manager'",
        )
        .bind(id)
        .bind(actor.id)
        .fetch_one(&s.db)
        .await?;
        if team.get::<i64, _>("owner_id") != actor.id && is_manager == 0 {
            return Err(ApiError::Forbidden);
        }
    }
    if !["manager", "developer", "reviewer"].contains(&req.role.as_str()) {
        return Err(ApiError::BadRequest("无效团队角色".into()));
    }
    if !req.weekly_capacity_hours.is_finite() || req.weekly_capacity_hours <= 0.0 {
        return Err(ApiError::BadRequest("周可用工时必须大于0".into()));
    }
    let user_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id=?")
        .bind(req.user_id)
        .fetch_one(&s.db)
        .await?;
    if user_exists == 0 {
        return Err(ApiError::BadRequest("用户不存在".into()));
    }
    sqlx::query("INSERT INTO team_members(team_id,user_id,role,weekly_capacity_hours) VALUES(?,?,?,?) ON CONFLICT(team_id,user_id) DO UPDATE SET role=excluded.role,weekly_capacity_hours=excluded.weekly_capacity_hours")
        .bind(id).bind(req.user_id).bind(req.role).bind(req.weekly_capacity_hours).execute(&s.db).await?;
    log(
        &s.db,
        actor.id,
        "team",
        Some(id),
        "add_member",
        &req.user_id.to_string(),
    )
    .await;
    Ok(Json(json!({"ok":true})))
}

async fn list_projects(State(s): State<AppState>, actor: Actor) -> ApiResult<Json<Value>> {
    let rows = if actor.role == "admin" {
        sqlx::query("SELECT p.*,u.display_name manager_name,(SELECT COALESCE(AVG(progress),0.0) FROM tasks WHERE project_id=p.id) completion_rate FROM projects p JOIN users u ON u.id=p.manager_id ORDER BY p.id DESC").fetch_all(&s.db).await?
    } else {
        sqlx::query("SELECT p.*,u.display_name manager_name,(SELECT COALESCE(AVG(progress),0.0) FROM tasks WHERE project_id=p.id) completion_rate FROM projects p JOIN users u ON u.id=p.manager_id JOIN project_members pm ON pm.project_id=p.id AND pm.user_id=? ORDER BY p.id DESC").bind(actor.id).fetch_all(&s.db).await?
    };
    Ok(Json(Value::Array(rows.iter().map(project_json).collect())))
}
fn project_json(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<i64,_>("id"),"team_id":r.get::<i64,_>("team_id"),"name":r.get::<String,_>("name"),"description":r.get::<String,_>("description"),"manager_id":r.get::<i64,_>("manager_id"),"manager_name":r.get::<String,_>("manager_name"),"start_date":r.get::<String,_>("start_date"),"end_date":r.get::<String,_>("end_date"),"budget":cents_to_yuan(r.get("budget_cents")),"budget_cents":r.get::<i64,_>("budget_cents"),"status":r.get::<String,_>("status"),"completion_rate":r.get::<f64,_>("completion_rate")})
}
async fn create_project(
    State(s): State<AppState>,
    actor: Actor,
    Json(req): Json<CreateProject>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    auth::require(&actor, &["admin", "manager"])?;
    if actor.role != "admin" {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM team_members WHERE team_id=? AND user_id=? AND role='manager'",
        )
        .bind(req.team_id)
        .bind(actor.id)
        .fetch_one(&s.db)
        .await?;
        if count == 0 {
            return Err(ApiError::Forbidden);
        }
    }
    nonempty(&req.name, "项目名称")?;
    if valid_date(&req.start_date)? > valid_date(&req.end_date)? || req.budget_cents < 0 {
        return Err(ApiError::BadRequest("项目日期或预算无效".into()));
    }
    let manager_in_team: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM team_members WHERE team_id=? AND user_id=?")
            .bind(req.team_id)
            .bind(req.manager_id)
            .fetch_one(&s.db)
            .await?;
    if manager_in_team == 0 {
        return Err(ApiError::BadRequest("项目经理必须属于项目团队".into()));
    }
    let mut tx = s.db.begin().await?;
    let out=sqlx::query("INSERT INTO projects(team_id,name,description,manager_id,start_date,end_date,budget_cents) VALUES(?,?,?,?,?,?,?)").bind(req.team_id).bind(&req.name).bind(req.description).bind(req.manager_id).bind(req.start_date).bind(req.end_date).bind(req.budget_cents).execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO project_members(project_id,user_id,project_role) VALUES(?,?,'manager')",
    )
    .bind(out.last_insert_rowid())
    .bind(req.manager_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    log(
        &s.db,
        actor.id,
        "project",
        Some(out.last_insert_rowid()),
        "create",
        &req.name,
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id":out.last_insert_rowid()})),
    ))
}
async fn get_project(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_project_member(&s.db, &actor, id).await?;
    let r=sqlx::query("SELECT p.*,u.display_name manager_name,(SELECT COALESCE(AVG(progress),0.0) FROM tasks WHERE project_id=p.id) completion_rate FROM projects p JOIN users u ON u.id=p.manager_id WHERE p.id=?").bind(id).fetch_optional(&s.db).await?.ok_or(ApiError::NotFound)?;
    Ok(Json(project_json(&r)))
}
async fn update_project(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
    Json(req): Json<UpdateProject>,
) -> ApiResult<Json<Value>> {
    require_project_manager(&s.db, &actor, id).await?;
    if req.budget_cents.is_some_and(|v| v < 0) {
        return Err(ApiError::BadRequest("预算不能为负数".into()));
    }
    sqlx::query("UPDATE projects SET name=COALESCE(?,name),description=COALESCE(?,description),start_date=COALESCE(?,start_date),end_date=COALESCE(?,end_date),budget_cents=COALESCE(?,budget_cents),status=COALESCE(?,status) WHERE id=?")
        .bind(req.name).bind(req.description).bind(req.start_date).bind(req.end_date).bind(req.budget_cents).bind(req.status).bind(id).execute(&s.db).await?;
    log(&s.db, actor.id, "project", Some(id), "update", "").await;
    Ok(Json(json!({"ok":true})))
}
async fn project_members(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_project_member(&s.db, &actor, id).await?;
    let rows=sqlx::query("SELECT u.id,u.username,u.display_name,u.role,u.hourly_rate_cents,u.active,pm.project_role,(SELECT COALESCE(SUM(t.estimated_hours),0.0) FROM tasks t WHERE t.project_id=pm.project_id AND t.assignee_id=u.id AND t.status<>'done') assigned_hours,(SELECT COUNT(*) FROM tasks t WHERE t.project_id=pm.project_id AND t.assignee_id=u.id AND t.status NOT IN ('todo','done')) active_tasks,COALESCE(tm.weekly_capacity_hours,40.0) capacity FROM project_members pm JOIN users u ON u.id=pm.user_id JOIN projects p ON p.id=pm.project_id LEFT JOIN team_members tm ON tm.team_id=p.team_id AND tm.user_id=u.id WHERE pm.project_id=?").bind(id).fetch_all(&s.db).await?;
    Ok(Json(Value::Array(
        rows.iter()
            .map(|r| {
                let assigned: f64 = r.get("assigned_hours");
                let capacity: f64 = r.get("capacity");
                let mut v = user_json(r);
                v["project_role"] = json!(r.get::<String, _>("project_role"));
                v["assigned_hours"] = json!(assigned);
                v["active_tasks"] = json!(r.get::<i64, _>("active_tasks"));
                v["load_rate"] = json!(assigned / capacity * 100.0);
                v
            })
            .collect(),
    )))
}
async fn add_project_member(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
    Json(req): Json<AddProjectMember>,
) -> ApiResult<Json<Value>> {
    require_project_manager(&s.db, &actor, id).await?;
    if !["manager", "developer", "reviewer"].contains(&req.project_role.as_str()) {
        return Err(ApiError::BadRequest("无效项目角色".into()));
    }
    let team_id: i64 = sqlx::query_scalar("SELECT team_id FROM projects WHERE id=?")
        .bind(id)
        .fetch_one(&s.db)
        .await?;
    let in_team: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM team_members WHERE team_id=? AND user_id=?")
            .bind(team_id)
            .bind(req.user_id)
            .fetch_one(&s.db)
            .await?;
    if in_team == 0 {
        return Err(ApiError::BadRequest("用户必须先加入项目所属团队".into()));
    }
    sqlx::query("INSERT INTO project_members(project_id,user_id,project_role) VALUES(?,?,?) ON CONFLICT(project_id,user_id) DO UPDATE SET project_role=excluded.project_role").bind(id).bind(req.user_id).bind(req.project_role).execute(&s.db).await?;
    log(
        &s.db,
        actor.id,
        "project",
        Some(id),
        "add_member",
        &req.user_id.to_string(),
    )
    .await;
    Ok(Json(json!({"ok":true})))
}

async fn list_milestones(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_project_member(&s.db, &actor, id).await?;
    let rows=sqlx::query("SELECT m.*,(SELECT COALESCE(AVG(progress),0.0) FROM tasks WHERE milestone_id=m.id) completion_rate FROM milestones m WHERE project_id=? ORDER BY start_date").bind(id).fetch_all(&s.db).await?;
    Ok(Json(Value::Array(rows.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"project_id":r.get::<i64,_>("project_id"),"name":r.get::<String,_>("name"),"description":r.get::<String,_>("description"),"start_date":r.get::<String,_>("start_date"),"end_date":r.get::<String,_>("end_date"),"due_date":r.get::<String,_>("end_date"),"status":r.get::<String,_>("status"),"completion_rate":r.get::<f64,_>("completion_rate")})).collect())))
}
async fn create_milestone(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
    Json(req): Json<CreateMilestone>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_project_manager(&s.db, &actor, id).await?;
    nonempty(&req.name, "里程碑名称")?;
    if valid_date(&req.start_date)? > valid_date(&req.end_date)? {
        return Err(ApiError::BadRequest(
            "里程碑结束日期不能早于开始日期".into(),
        ));
    }
    let out = sqlx::query(
        "INSERT INTO milestones(project_id,name,description,start_date,end_date) VALUES(?,?,?,?,?)",
    )
    .bind(id)
    .bind(&req.name)
    .bind(req.description)
    .bind(req.start_date)
    .bind(req.end_date)
    .execute(&s.db)
    .await?;
    log(
        &s.db,
        actor.id,
        "milestone",
        Some(out.last_insert_rowid()),
        "create",
        &req.name,
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id":out.last_insert_rowid()})),
    ))
}

fn task_json(r: &sqlx::sqlite::SqliteRow) -> Value {
    let p: String = r.get("priority");
    json!({"id":r.get::<i64,_>("id"),"project_id":r.get::<i64,_>("project_id"),"milestone_id":r.get::<Option<i64>,_>("milestone_id"),"title":r.get::<String,_>("title"),"description":r.get::<String,_>("description"),"acceptance_criteria":r.get::<String,_>("acceptance_criteria"),"assignee_id":r.get::<Option<i64>,_>("assignee_id"),"assignee_name":r.get::<Option<String>,_>("assignee_name"),"priority":p,"status":r.get::<String,_>("status"),"start_date":r.get::<String,_>("planned_start"),"due_date":r.get::<String,_>("planned_end"),"planned_start":r.get::<String,_>("planned_start"),"planned_end":r.get::<String,_>("planned_end"),"planned_hours":r.get::<f64,_>("estimated_hours"),"estimated_hours":r.get::<f64,_>("estimated_hours"),"progress":r.get::<i64,_>("progress"),"actual_hours":r.get::<f64,_>("actual_hours"),"blocked":r.get::<i64,_>("blocked_count")>0})
}
const TASK_SELECT: &str = "SELECT t.*,u.display_name assignee_name,(SELECT COALESCE(SUM(hours),0.0) FROM worklogs WHERE task_id=t.id) actual_hours,(SELECT COUNT(*) FROM task_dependencies d JOIN tasks p ON p.id=d.depends_on_task_id WHERE d.task_id=t.id AND p.status<>'done') blocked_count FROM tasks t LEFT JOIN users u ON u.id=t.assignee_id";
async fn list_tasks(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_project_member(&s.db, &actor, id).await?;
    let rows = sqlx::query(&format!("{TASK_SELECT} WHERE t.project_id=? ORDER BY t.id"))
        .bind(id)
        .fetch_all(&s.db)
        .await?;
    Ok(Json(Value::Array(rows.iter().map(task_json).collect())))
}
async fn get_task(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let project_id = task_project_id(&s.db, id).await?;
    require_project_member(&s.db, &actor, project_id).await?;
    let r = sqlx::query(&format!("{TASK_SELECT} WHERE t.id=?"))
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    let mut v = task_json(&r);
    let parts=sqlx::query("SELECT u.id,u.display_name name FROM task_participants tp JOIN users u ON u.id=tp.user_id WHERE tp.task_id=?").bind(id).fetch_all(&s.db).await?;
    v["participants"] = Value::Array(
        parts
            .iter()
            .map(|x| json!({"id":x.get::<i64,_>("id"),"name":x.get::<String,_>("name")}))
            .collect(),
    );
    let reviews = sqlx::query("SELECT r.id,r.stage,r.result,r.comment,r.created_at,u.id reviewer_id,u.display_name reviewer_name FROM task_reviews r JOIN users u ON u.id=r.reviewer_id WHERE r.task_id=? ORDER BY r.id")
        .bind(id).fetch_all(&s.db).await?;
    v["reviews"] = Value::Array(reviews.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"stage":r.get::<String,_>("stage"),"result":r.get::<String,_>("result"),"comment":r.get::<String,_>("comment"),"created_at":r.get::<String,_>("created_at"),"reviewer_id":r.get::<i64,_>("reviewer_id"),"reviewer_name":r.get::<String,_>("reviewer_name")})).collect());
    let history = sqlx::query("SELECT l.id,l.action,l.detail,l.created_at,u.display_name actor_name FROM activity_logs l LEFT JOIN users u ON u.id=l.actor_id WHERE l.entity_type='task' AND l.entity_id=? ORDER BY l.id DESC")
        .bind(id).fetch_all(&s.db).await?;
    v["activity_logs"] = Value::Array(history.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"action":r.get::<String,_>("action"),"detail":r.get::<String,_>("detail"),"created_at":r.get::<String,_>("created_at"),"actor_name":r.get::<Option<String>,_>("actor_name")})).collect());
    Ok(Json(v))
}
async fn create_task(
    State(s): State<AppState>,
    actor: Actor,
    Path(project_id): Path<i64>,
    Json(req): Json<CreateTask>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_project_manager(&s.db, &actor, project_id).await?;
    nonempty(&req.title, "任务名称")?;
    if !["low", "medium", "high", "critical"].contains(&priority_in(&req.priority)) {
        return Err(ApiError::BadRequest("无效任务优先级".into()));
    }
    if valid_date(&req.planned_start)? > valid_date(&req.planned_end)?
        || !req.estimated_hours.is_finite()
        || req.estimated_hours < 0.0
    {
        return Err(ApiError::BadRequest("任务日期或工时无效".into()));
    }
    validate_task_relations(
        &s.db,
        project_id,
        req.milestone_id,
        req.assignee_id,
        &req.participant_ids,
    )
    .await?;
    let mut tx = s.db.begin().await?;
    let out=sqlx::query("INSERT INTO tasks(project_id,milestone_id,title,description,acceptance_criteria,assignee_id,priority,planned_start,planned_end,estimated_hours) VALUES(?,?,?,?,?,?,?,?,?,?)").bind(project_id).bind(req.milestone_id).bind(&req.title).bind(req.description).bind(req.acceptance_criteria).bind(req.assignee_id).bind(priority_in(&req.priority)).bind(req.planned_start).bind(req.planned_end).bind(req.estimated_hours).execute(&mut *tx).await?;
    for uid in req.participant_ids {
        sqlx::query("INSERT OR IGNORE INTO task_participants(task_id,user_id) VALUES(?,?)")
            .bind(out.last_insert_rowid())
            .bind(uid)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    log(
        &s.db,
        actor.id,
        "task",
        Some(out.last_insert_rowid()),
        "create",
        &req.title,
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id":out.last_insert_rowid()})),
    ))
}
async fn update_task(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
    Json(req): Json<UpdateTask>,
) -> ApiResult<Json<Value>> {
    let project_id = require_task_contributor(&s.db, &actor, id).await?;
    if let Some(title) = &req.title {
        nonempty(title, "任务名称")?
    }
    if req
        .priority
        .as_deref()
        .is_some_and(|p| !["low", "medium", "high", "critical"].contains(&priority_in(p)))
    {
        return Err(ApiError::BadRequest("无效任务优先级".into()));
    }
    if req.progress.is_some_and(|p| !(0..=100).contains(&p)) {
        return Err(ApiError::BadRequest("任务进度必须在0到100之间".into()));
    }
    if req
        .estimated_hours
        .is_some_and(|h| !h.is_finite() || h < 0.0)
    {
        return Err(ApiError::BadRequest("预计工时无效".into()));
    }
    let dates = sqlx::query("SELECT planned_start,planned_end FROM tasks WHERE id=?")
        .bind(id)
        .fetch_one(&s.db)
        .await?;
    let current_start: String = dates.get("planned_start");
    let current_end: String = dates.get("planned_end");
    let start = req.planned_start.as_deref().unwrap_or(&current_start);
    let end = req.planned_end.as_deref().unwrap_or(&current_end);
    if valid_date(start)? > valid_date(end)? {
        return Err(ApiError::BadRequest("任务结束日期不能早于开始日期".into()));
    }
    let role = project_role(&s.db, &actor, project_id).await?;
    if role == "developer"
        && (req.milestone_id.is_some()
            || req.assignee_id.is_some()
            || req.participant_ids.is_some()
            || req.priority.is_some()
            || req.planned_start.is_some()
            || req.planned_end.is_some()
            || req.estimated_hours.is_some())
    {
        return Err(ApiError::Forbidden);
    }
    validate_task_relations(
        &s.db,
        project_id,
        req.milestone_id,
        req.assignee_id,
        req.participant_ids.as_deref().unwrap_or(&[]),
    )
    .await?;
    let participants = req.participant_ids.clone();
    let mut tx = s.db.begin().await?;
    sqlx::query("UPDATE tasks SET milestone_id=COALESCE(?,milestone_id),title=COALESCE(?,title),description=COALESCE(?,description),acceptance_criteria=COALESCE(?,acceptance_criteria),assignee_id=COALESCE(?,assignee_id),priority=COALESCE(?,priority),planned_start=COALESCE(?,planned_start),planned_end=COALESCE(?,planned_end),estimated_hours=COALESCE(?,estimated_hours),progress=COALESCE(?,progress),updated_at=CURRENT_TIMESTAMP WHERE id=?")
        .bind(req.milestone_id).bind(req.title).bind(req.description).bind(req.acceptance_criteria).bind(req.assignee_id).bind(req.priority.as_deref().map(priority_in)).bind(req.planned_start).bind(req.planned_end).bind(req.estimated_hours).bind(req.progress).bind(id).execute(&mut *tx).await?;
    if let Some(participants) = participants {
        sqlx::query("DELETE FROM task_participants WHERE task_id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for uid in participants {
            sqlx::query("INSERT INTO task_participants(task_id,user_id) VALUES(?,?)")
                .bind(id)
                .bind(uid)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    log(&s.db, actor.id, "task", Some(id), "update", "").await;
    Ok(Json(json!({"ok":true})))
}
async fn transition_task(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
    Json(req): Json<TransitionTask>,
) -> ApiResult<Json<Value>> {
    let project_id = require_task_contributor(&s.db, &actor, id).await?;
    let effective_role = project_role(&s.db, &actor, project_id).await?;
    let r = sqlx::query("SELECT status FROM tasks WHERE id=?")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    let from: String = r.get("status");
    let from_status = TaskStatus::parse(&from)?;
    let to = TaskStatus::parse(&req.to_status)?;
    if !services::valid_transition(from_status, to, &effective_role) {
        return Err(ApiError::BadRequest(format!(
            "角色{}不允许从{}流转到{}",
            effective_role, from, req.to_status
        )));
    }
    if req.to_status == "in_progress" {
        let blocked:i64=sqlx::query_scalar("SELECT COUNT(*) FROM task_dependencies d JOIN tasks p ON p.id=d.depends_on_task_id WHERE d.task_id=? AND p.status<>'done'").bind(id).fetch_one(&s.db).await?;
        if blocked > 0 {
            return Err(ApiError::BadRequest("前置任务尚未完成".into()));
        }
    }
    let progress = if req.to_status == "done" {
        100
    } else if req.to_status == "in_progress" && from == "todo" {
        1
    } else {
        -1
    };
    if progress >= 0 {
        sqlx::query("UPDATE tasks SET status=?,progress=?,updated_at=CURRENT_TIMESTAMP WHERE id=?")
            .bind(&req.to_status)
            .bind(progress)
            .bind(id)
            .execute(&s.db)
            .await?;
    } else {
        sqlx::query("UPDATE tasks SET status=?,updated_at=CURRENT_TIMESTAMP WHERE id=?")
            .bind(&req.to_status)
            .bind(id)
            .execute(&s.db)
            .await?;
    }
    log(
        &s.db,
        actor.id,
        "task",
        Some(id),
        "transition",
        &format!("{from} -> {}", req.to_status),
    )
    .await;
    Ok(Json(json!({"ok":true,"from":from,"to":req.to_status})))
}
async fn task_comments(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_project_member(&s.db, &actor, task_project_id(&s.db, id).await?).await?;
    let rows=sqlx::query("SELECT c.id,c.content,c.created_at,u.id author_id,u.display_name author_name FROM task_comments c JOIN users u ON u.id=c.author_id WHERE c.task_id=? ORDER BY c.id").bind(id).fetch_all(&s.db).await?;
    Ok(Json(Value::Array(rows.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"content":r.get::<String,_>("content"),"created_at":r.get::<String,_>("created_at"),"author_id":r.get::<i64,_>("author_id"),"author_name":r.get::<String,_>("author_name")})).collect())))
}
async fn add_comment(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
    Json(req): Json<CreateComment>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_task_contributor(&s.db, &actor, id).await?;
    nonempty(&req.content, "评论")?;
    let o = sqlx::query("INSERT INTO task_comments(task_id,author_id,content) VALUES(?,?,?)")
        .bind(id)
        .bind(actor.id)
        .bind(req.content)
        .execute(&s.db)
        .await?;
    log(&s.db, actor.id, "task", Some(id), "comment", "").await;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id":o.last_insert_rowid()})),
    ))
}
async fn review_task(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
    Json(req): Json<ReviewTask>,
) -> ApiResult<Json<Value>> {
    require_task_reviewer(&s.db, &actor, id).await?;
    services::validate_review_input(&req.result, &req.comment)?;
    let status: String = sqlx::query_scalar("SELECT status FROM tasks WHERE id=?")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    let stage = match status.as_str() {
        "review" => "review",
        "testing" => "testing",
        _ => return Err(ApiError::BadRequest("任务当前不在评审或测试阶段".into())),
    };
    let to = match (stage, req.result.as_str()) {
        ("review", "approved") => "testing",
        ("testing", "approved") => "done",
        (_, "rejected") => "in_progress",
        _ => unreachable!(),
    };
    let mut tx = s.db.begin().await?;
    sqlx::query(
        "INSERT INTO task_reviews(task_id,reviewer_id,stage,result,comment) VALUES(?,?,?,?,?)",
    )
    .bind(id)
    .bind(actor.id)
    .bind(stage)
    .bind(&req.result)
    .bind(&req.comment)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE tasks SET status=?,progress=CASE WHEN ?='done' THEN 100 ELSE progress END,updated_at=CURRENT_TIMESTAMP WHERE id=?").bind(to).bind(to).bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    log(
        &s.db,
        actor.id,
        "task",
        Some(id),
        "review",
        &format!("{} -> {to}", req.result),
    )
    .await;
    Ok(Json(json!({"ok":true,"to_status":to})))
}
async fn task_dependencies(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_project_member(&s.db, &actor, task_project_id(&s.db, id).await?).await?;
    let rows=sqlx::query("SELECT t.id,t.title,t.status FROM task_dependencies d JOIN tasks t ON t.id=d.depends_on_task_id WHERE d.task_id=?").bind(id).fetch_all(&s.db).await?;
    Ok(Json(Value::Array(rows.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"title":r.get::<String,_>("title"),"status":r.get::<String,_>("status")})).collect())))
}
async fn add_dependency(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
    Json(req): Json<AddDependency>,
) -> ApiResult<Json<Value>> {
    let project_id: i64 = sqlx::query_scalar("SELECT project_id FROM tasks WHERE id=?")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    require_project_manager(&s.db, &actor, project_id).await?;
    let dep_project: Option<i64> = sqlx::query_scalar("SELECT project_id FROM tasks WHERE id=?")
        .bind(req.depends_on_task_id)
        .fetch_optional(&s.db)
        .await?;
    if dep_project != Some(project_id) {
        return Err(ApiError::BadRequest("依赖任务必须属于同一项目".into()));
    }
    let rows=sqlx::query("SELECT task_id,depends_on_task_id FROM task_dependencies d JOIN tasks t ON t.id=d.task_id WHERE t.project_id=?").bind(project_id).fetch_all(&s.db).await?;
    let edges: Vec<(i64, i64)> = rows.iter().map(|r| (r.get(0), r.get(1))).collect();
    if services::would_create_cycle(&edges, id, req.depends_on_task_id) {
        return Err(ApiError::BadRequest("该依赖会形成循环".into()));
    }
    sqlx::query("INSERT OR IGNORE INTO task_dependencies(task_id,depends_on_task_id) VALUES(?,?)")
        .bind(id)
        .bind(req.depends_on_task_id)
        .execute(&s.db)
        .await?;
    log(
        &s.db,
        actor.id,
        "task",
        Some(id),
        "add_dependency",
        &req.depends_on_task_id.to_string(),
    )
    .await;
    Ok(Json(json!({"ok":true})))
}
async fn remove_dependency(
    State(s): State<AppState>,
    actor: Actor,
    Path((id, dependency_id)): Path<(i64, i64)>,
) -> ApiResult<Json<Value>> {
    let project_id = task_project_id(&s.db, id).await?;
    require_project_manager(&s.db, &actor, project_id).await?;
    sqlx::query("DELETE FROM task_dependencies WHERE task_id=? AND depends_on_task_id=?")
        .bind(id)
        .bind(dependency_id)
        .execute(&s.db)
        .await?;
    log(
        &s.db,
        actor.id,
        "task",
        Some(id),
        "remove_dependency",
        &dependency_id.to_string(),
    )
    .await;
    Ok(Json(json!({"ok":true})))
}
async fn task_downstream(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let pid: i64 = sqlx::query_scalar("SELECT project_id FROM tasks WHERE id=?")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    require_project_member(&s.db, &actor, pid).await?;
    let rows=sqlx::query("SELECT task_id,depends_on_task_id FROM task_dependencies d JOIN tasks t ON t.id=d.task_id WHERE t.project_id=?").bind(pid).fetch_all(&s.db).await?;
    let edges = rows
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect::<Vec<_>>();
    let ids = services::downstream(&edges, id);
    Ok(Json(
        json!({"task_id":id,"affected_task_ids":ids,"affected_count":ids.len()}),
    ))
}

async fn create_worklog_for_project(
    State(s): State<AppState>,
    actor: Actor,
    Path(pid): Path<i64>,
    Json(req): Json<CreateWorklog>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    insert_worklog(&s, &actor, req, Some(pid)).await
}
async fn insert_worklog(
    s: &AppState,
    actor: &Actor,
    req: CreateWorklog,
    pid: Option<i64>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    if req.hours <= 0.0 || req.hours > 24.0 {
        return Err(ApiError::BadRequest("单日工时须大于0且不超过24".into()));
    }
    valid_date(&req.work_date)?;
    let actual = task_project_id(&s.db, req.task_id).await?;
    require_task_contributor(&s.db, actor, req.task_id).await?;
    if let Some(pid) = pid
        && actual != pid
    {
        return Err(ApiError::BadRequest("任务不属于该项目".into()));
    }
    let o = sqlx::query(
        "INSERT INTO worklogs(task_id,user_id,work_date,hours,content) VALUES(?,?,?,?,?)",
    )
    .bind(req.task_id)
    .bind(actor.id)
    .bind(req.work_date)
    .bind(req.hours)
    .bind(req.content)
    .execute(&s.db)
    .await?;
    log(
        &s.db,
        actor.id,
        "worklog",
        Some(o.last_insert_rowid()),
        "create",
        "",
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id":o.last_insert_rowid()})),
    ))
}
async fn project_worklogs(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_project_member(&s.db, &actor, id).await?;
    let rows=sqlx::query("SELECT w.id,w.task_id,t.title task_title,w.user_id,u.display_name user_name,w.work_date,w.hours,w.content,(w.hours*u.hourly_rate_cents) cost_cents FROM worklogs w JOIN tasks t ON t.id=w.task_id JOIN users u ON u.id=w.user_id WHERE t.project_id=? ORDER BY w.work_date DESC,w.id DESC").bind(id).fetch_all(&s.db).await?;
    Ok(Json(Value::Array(rows.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"task_id":r.get::<i64,_>("task_id"),"task_title":r.get::<String,_>("task_title"),"user_id":r.get::<i64,_>("user_id"),"user_name":r.get::<String,_>("user_name"),"work_date":r.get::<String,_>("work_date"),"hours":r.get::<f64,_>("hours"),"content":r.get::<String,_>("content"),"cost":cents_to_yuan(r.get::<f64,_>("cost_cents").round() as i64)})).collect())))
}
async fn project_expenses(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_project_member(&s.db, &actor, id).await?;
    let rows=sqlx::query("SELECT e.*,u.display_name creator_name FROM expenses e JOIN users u ON u.id=e.created_by WHERE project_id=? ORDER BY occurred_on DESC,id DESC").bind(id).fetch_all(&s.db).await?;
    Ok(Json(Value::Array(rows.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"category":r.get::<String,_>("category"),"amount":cents_to_yuan(r.get("amount_cents")),"amount_cents":r.get::<i64,_>("amount_cents"),"occurred_on":r.get::<String,_>("occurred_on"),"description":r.get::<String,_>("description"),"created_by":r.get::<i64,_>("created_by"),"creator_name":r.get::<String,_>("creator_name")})).collect())))
}
async fn create_expense(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
    Json(req): Json<CreateExpense>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_project_manager(&s.db, &actor, id).await?;
    if req.amount_cents < 0
        || !["device", "cloud", "purchase", "other"].contains(&req.category.as_str())
    {
        return Err(ApiError::BadRequest("支出类型或金额无效".into()));
    }
    valid_date(&req.occurred_on)?;
    let o=sqlx::query("INSERT INTO expenses(project_id,category,amount_cents,occurred_on,description,created_by) VALUES(?,?,?,?,?,?)").bind(id).bind(req.category).bind(req.amount_cents).bind(req.occurred_on).bind(req.description).bind(actor.id).execute(&s.db).await?;
    log(
        &s.db,
        actor.id,
        "expense",
        Some(o.last_insert_rowid()),
        "create",
        "",
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id":o.last_insert_rowid()})),
    ))
}

async fn cost_values(db: &SqlitePool, pid: i64) -> ApiResult<(i64, i64, i64)> {
    let budget: i64 = sqlx::query_scalar("SELECT budget_cents FROM projects WHERE id=?")
        .bind(pid)
        .fetch_optional(db)
        .await?
        .ok_or(ApiError::NotFound)?;
    let labor:f64=sqlx::query_scalar("SELECT COALESCE(SUM(w.hours*u.hourly_rate_cents),0.0) FROM worklogs w JOIN tasks t ON t.id=w.task_id JOIN users u ON u.id=w.user_id WHERE t.project_id=?").bind(pid).fetch_one(db).await?;
    let expense: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(amount_cents),0) FROM expenses WHERE project_id=?")
            .bind(pid)
            .fetch_one(db)
            .await?;
    Ok((budget, labor.round() as i64, expense))
}
async fn evm_for(db: &SqlitePool, pid: i64) -> ApiResult<services::EvmMetrics> {
    let (budget, labor, expense) = cost_values(db, pid).await?;
    let tasks = sqlx::query(
        "SELECT progress,planned_start,planned_end,estimated_hours FROM tasks WHERE project_id=?",
    )
    .bind(pid)
    .fetch_all(db)
    .await?;
    let total_hours: f64 = tasks
        .iter()
        .map(|r| r.get::<f64, _>("estimated_hours"))
        .sum();
    let today = Utc::now().date_naive();
    let mut pv = 0.0;
    let mut ev = 0.0;
    for r in tasks {
        let share = if total_hours > 0.0 {
            budget as f64 * r.get::<f64, _>("estimated_hours") / total_hours
        } else {
            0.0
        };
        let start = valid_date(r.get("planned_start"))?;
        let end = valid_date(r.get("planned_end"))?;
        let planned = if today < start {
            0.0
        } else if today >= end {
            1.0
        } else {
            (today - start).num_days() as f64 / (end - start).num_days().max(1) as f64
        };
        pv += share * planned;
        ev += share * r.get::<i64, _>("progress") as f64 / 100.0;
    }
    Ok(services::calculate_evm(
        pv.round() as i64,
        ev.round() as i64,
        labor + expense,
        budget,
    ))
}
async fn finance_summary(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_project_member(&s.db, &actor, id).await?;
    let (budget, labor, expense) = cost_values(&s.db, id).await?;
    let evm = evm_for(&s.db, id).await?;
    let category_rows = sqlx::query(
        "SELECT category,COALESCE(SUM(amount_cents),0) amount_cents FROM expenses WHERE project_id=? GROUP BY category",
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    let mut device = 0_i64;
    let mut cloud = 0_i64;
    let mut purchase = 0_i64;
    let mut other = 0_i64;
    for row in category_rows {
        let amount: i64 = row.get("amount_cents");
        match row.get::<String, _>("category").as_str() {
            "device" => device = amount,
            "cloud" => cloud = amount,
            "purchase" => purchase = amount,
            _ => other = amount,
        }
    }
    Ok(Json(
        json!({"budget":cents_to_yuan(budget),"labor_cost":cents_to_yuan(labor),"equipment_cost":cents_to_yuan(device),"device_cost":cents_to_yuan(device),"cloud_cost":cents_to_yuan(cloud),"procurement_cost":cents_to_yuan(purchase),"purchase_cost":cents_to_yuan(purchase),"other_cost":cents_to_yuan(other),"expense_cost":cents_to_yuan(expense),"actual_cost":cents_to_yuan(labor+expense),"remaining_budget":cents_to_yuan(budget-labor-expense),"budget_usage_rate":if budget>0{(labor+expense)as f64/budget as f64*100.0}else{0.0},"expense_breakdown":{"device":cents_to_yuan(device),"cloud":cents_to_yuan(cloud),"purchase":cents_to_yuan(purchase),"other":cents_to_yuan(other)},"evm":evm}),
    ))
}
async fn project_metrics(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    finance_summary(State(s), actor, Path(id)).await
}

async fn risk_values(db: &SqlitePool, pid: i64) -> ApiResult<Vec<Value>> {
    let today = Utc::now().date_naive().format("%Y-%m-%d").to_string();
    let tasks=sqlx::query("SELECT t.id,t.title,t.planned_end,t.status,t.assignee_id,u.display_name assignee_name FROM tasks t LEFT JOIN users u ON u.id=t.assignee_id WHERE t.project_id=?").bind(pid).fetch_all(db).await?;
    let edges_rows=sqlx::query("SELECT task_id,depends_on_task_id FROM task_dependencies d JOIN tasks t ON t.id=d.task_id WHERE t.project_id=?").bind(pid).fetch_all(db).await?;
    let edges = edges_rows
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect::<Vec<_>>();
    let mut out = Vec::new();
    for r in tasks {
        let status: String = r.get("status");
        let end: String = r.get("planned_end");
        if status != "done" && end < today {
            let task_id: i64 = r.get("id");
            let affected = services::downstream(&edges, task_id);
            let mut affected_tasks = Vec::new();
            for affected_id in &affected {
                if let Some(item)=sqlx::query("SELECT t.id,t.title,t.assignee_id,u.display_name assignee_name FROM tasks t LEFT JOIN users u ON u.id=t.assignee_id WHERE t.id=?").bind(affected_id).fetch_optional(db).await?{affected_tasks.push(json!({"id":item.get::<i64,_>("id"),"title":item.get::<String,_>("title"),"assignee_id":item.get::<Option<i64>,_>("assignee_id"),"assignee_name":item.get::<Option<String>,_>("assignee_name")}));}
            }
            out.push(json!({"kind":"overdue","level":if affected.is_empty(){"medium"}else{"high"},"task_id":task_id,"task_title":r.get::<String,_>("title"),"message":format!("任务已逾期，影响{}个下游任务",affected.len()),"affected_task_ids":affected,"affected_tasks":affected_tasks,"assignee_id":r.get::<Option<i64>,_>("assignee_id"),"assignee_name":r.get::<Option<String>,_>("assignee_name")}));
        }
    }
    let blocked = sqlx::query("SELECT DISTINCT t.id,t.title,p.id dependency_id,p.title dependency_title FROM tasks t JOIN task_dependencies d ON d.task_id=t.id JOIN tasks p ON p.id=d.depends_on_task_id WHERE t.project_id=? AND t.status<>'done' AND p.status<>'done'")
        .bind(pid).fetch_all(db).await?;
    for r in blocked {
        out.push(json!({"kind":"dependency_blocked","level":"medium","task_id":r.get::<i64,_>("id"),"task_title":r.get::<String,_>("title"),"dependency_id":r.get::<i64,_>("dependency_id"),"message":format!("前置任务“{}”尚未完成",r.get::<String,_>("dependency_title"))}));
    }
    let members=sqlx::query("SELECT u.id,u.display_name,COALESCE(SUM(t.estimated_hours),0.0) assigned,COALESCE(tm.weekly_capacity_hours,40.0) capacity FROM project_members pm JOIN users u ON u.id=pm.user_id JOIN projects p ON p.id=pm.project_id LEFT JOIN team_members tm ON tm.team_id=p.team_id AND tm.user_id=u.id LEFT JOIN tasks t ON t.project_id=pm.project_id AND t.assignee_id=u.id AND t.status<>'done' WHERE pm.project_id=? GROUP BY u.id").bind(pid).fetch_all(db).await?;
    for r in members {
        let a: f64 = r.get("assigned");
        let c: f64 = r.get("capacity");
        if a > c {
            out.push(json!({"kind":"overload","level":"high","user_id":r.get::<i64,_>("id"),"user_name":r.get::<String,_>("display_name"),"message":format!("成员负载率{:.1}%",a/c*100.0)}));
        }
    }
    let (budget, labor, expense) = cost_values(db, pid).await?;
    if budget > 0 && labor + expense > budget {
        out.push(
            json!({"kind":"budget_overrun","level":"high","message":"项目实际成本已超过预算"}),
        );
    }
    let evm = evm_for(db, pid).await?;
    if evm.eac_cents.is_some_and(|v| v > budget) {
        out.push(
            json!({"kind":"forecast_overrun","level":"high","message":"EAC预测完工成本将超过预算"}),
        );
    }
    Ok(out)
}
async fn project_risks(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_project_member(&s.db, &actor, id).await?;
    Ok(Json(Value::Array(risk_values(&s.db, id).await?)))
}

#[derive(Deserialize)]
struct ProjectQuery {
    project_id: i64,
}
async fn global_finance_summary(
    State(s): State<AppState>,
    actor: Actor,
    Query(q): Query<ProjectQuery>,
) -> ApiResult<Json<Value>> {
    finance_summary(State(s), actor, Path(q.project_id)).await
}
async fn global_worklogs(
    State(s): State<AppState>,
    actor: Actor,
    Query(q): Query<ProjectQuery>,
) -> ApiResult<Json<Value>> {
    project_worklogs(State(s), actor, Path(q.project_id)).await
}
async fn global_create_worklog(
    State(s): State<AppState>,
    actor: Actor,
    Query(q): Query<ProjectQuery>,
    Json(req): Json<CreateWorklog>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    insert_worklog(&s, &actor, req, Some(q.project_id)).await
}
async fn global_expenses(
    State(s): State<AppState>,
    actor: Actor,
    Query(q): Query<ProjectQuery>,
) -> ApiResult<Json<Value>> {
    project_expenses(State(s), actor, Path(q.project_id)).await
}
async fn global_create_expense(
    State(s): State<AppState>,
    actor: Actor,
    Query(q): Query<ProjectQuery>,
    Json(req): Json<CreateExpense>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    create_expense(State(s), actor, Path(q.project_id), Json(req)).await
}
async fn global_risks(
    State(s): State<AppState>,
    actor: Actor,
    Query(q): Query<ProjectQuery>,
) -> ApiResult<Json<Value>> {
    project_risks(State(s), actor, Path(q.project_id)).await
}
async fn scan_risks(
    State(s): State<AppState>,
    actor: Actor,
    Json(q): Json<ProjectQuery>,
) -> ApiResult<Json<Value>> {
    require_project_manager(&s.db, &actor, q.project_id).await?;
    let risks = risk_values(&s.db, q.project_id).await?;
    let mut tx = s.db.begin().await?;
    sqlx::query("DELETE FROM risk_alerts WHERE project_id=? AND resolved=0")
        .bind(q.project_id)
        .execute(&mut *tx)
        .await?;
    for r in &risks {
        sqlx::query(
            "INSERT INTO risk_alerts(project_id,task_id,kind,level,message) VALUES(?,?,?,?,?)",
        )
        .bind(q.project_id)
        .bind(r["task_id"].as_i64())
        .bind(r["kind"].as_str().unwrap_or("unknown"))
        .bind(r["level"].as_str().unwrap_or("medium"))
        .bind(r["message"].as_str().unwrap_or(""))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    log(
        &s.db,
        actor.id,
        "risk",
        Some(q.project_id),
        "scan",
        &format!("{} alerts", risks.len()),
    )
    .await;
    Ok(Json(json!({"count":risks.len(),"risks":risks})))
}

async fn list_decisions(State(s): State<AppState>, actor: Actor) -> ApiResult<Json<Value>> {
    let rows = if actor.role == "admin" {
        sqlx::query("SELECT d.*,p.name project_name,o.name confirmed_option_name FROM decisions d JOIN projects p ON p.id=d.project_id LEFT JOIN decision_options o ON o.id=d.confirmed_option_id ORDER BY d.id DESC").fetch_all(&s.db).await?
    } else {
        sqlx::query("SELECT d.*,p.name project_name,o.name confirmed_option_name FROM decisions d JOIN projects p ON p.id=d.project_id JOIN project_members pm ON pm.project_id=d.project_id AND pm.user_id=? LEFT JOIN decision_options o ON o.id=d.confirmed_option_id ORDER BY d.id DESC").bind(actor.id).fetch_all(&s.db).await?
    };
    Ok(Json(Value::Array(rows.iter().map(decision_json).collect())))
}
fn decision_json(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<i64,_>("id"),"project_id":r.get::<i64,_>("project_id"),"project_name":r.try_get::<String,_>("project_name").ok(),"title":r.get::<String,_>("title"),"description":r.get::<String,_>("description"),"analysis_years":r.get::<i64,_>("analysis_years"),"status":r.get::<String,_>("status"),"confirmed_option_id":r.get::<Option<i64>,_>("confirmed_option_id"),"confirmed_option_name":r.try_get::<Option<String>,_>("confirmed_option_name").ok().flatten(),"confirmation_reason":r.get::<Option<String>,_>("confirmation_reason"),"created_at":r.get::<String,_>("created_at")})
}
async fn create_decision(
    State(s): State<AppState>,
    actor: Actor,
    Json(req): Json<CreateDecision>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_project_manager(&s.db, &actor, req.project_id).await?;
    nonempty(&req.title, "决策名称")?;
    if req.analysis_years <= 0 {
        return Err(ApiError::BadRequest("分析年限必须大于0".into()));
    }
    if req.options.len() < 2 {
        return Err(ApiError::BadRequest("至少需要两个候选方案".into()));
    }
    if req.metrics.is_empty() || req.metrics.iter().map(|m| m.weight_bps).sum::<i64>() != 10000 {
        return Err(ApiError::BadRequest("指标权重之和必须为10000基点".into()));
    }
    if req.metrics.iter().any(|m| {
        m.name.trim().is_empty()
            || !(1..=10000).contains(&m.weight_bps)
            || !["higher", "lower"].contains(&m.direction.as_str())
    }) {
        return Err(ApiError::BadRequest("指标名称、方向或权重无效".into()));
    }
    if req
        .options
        .iter()
        .any(|o| o.values.len() != req.metrics.len())
    {
        return Err(ApiError::BadRequest(
            "每个方案必须为全部指标提供数值".into(),
        ));
    }
    if req.options.iter().any(|o| {
        o.name.trim().is_empty()
            || o.initial_cost_cents < 0
            || o.development_cost_cents < 0
            || o.annual_operation_cost_cents < 0
            || o.risk_loss_cents < 0
            || o.expected_benefit_cents < 0
            || !(0..=10000).contains(&o.risk_probability_bps)
            || o.values.iter().any(|v| !v.is_finite())
    }) {
        return Err(ApiError::BadRequest(
            "候选方案名称、经济数据、风险概率或指标值无效".into(),
        ));
    }
    let mut tx = s.db.begin().await?;
    let d=sqlx::query("INSERT INTO decisions(project_id,title,description,analysis_years,created_by) VALUES(?,?,?,?,?)").bind(req.project_id).bind(&req.title).bind(req.description).bind(req.analysis_years).bind(actor.id).execute(&mut *tx).await?.last_insert_rowid();
    let mut mids = Vec::new();
    for m in req.metrics {
        if !["higher", "lower"].contains(&m.direction.as_str()) {
            return Err(ApiError::BadRequest("指标方向必须为higher或lower".into()));
        }
        let id=sqlx::query("INSERT INTO decision_metrics(decision_id,name,weight_bps,direction,unit) VALUES(?,?,?,?,?)").bind(d).bind(m.name).bind(m.weight_bps).bind(m.direction).bind(m.unit).execute(&mut *tx).await?.last_insert_rowid();
        mids.push(id);
    }
    for o in req.options {
        let oid=sqlx::query("INSERT INTO decision_options(decision_id,name,description,initial_cost_cents,development_cost_cents,annual_operation_cost_cents,risk_probability_bps,risk_loss_cents,expected_benefit_cents) VALUES(?,?,?,?,?,?,?,?,?)").bind(d).bind(o.name).bind(o.description).bind(o.initial_cost_cents).bind(o.development_cost_cents).bind(o.annual_operation_cost_cents).bind(o.risk_probability_bps).bind(o.risk_loss_cents).bind(o.expected_benefit_cents).execute(&mut *tx).await?.last_insert_rowid();
        for (mid, value) in mids.iter().zip(o.values) {
            sqlx::query("INSERT INTO decision_values(option_id,metric_id,raw_value) VALUES(?,?,?)")
                .bind(oid)
                .bind(mid)
                .bind(value)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    log(&s.db, actor.id, "decision", Some(d), "create", &req.title).await;
    Ok((StatusCode::CREATED, Json(json!({"id":d}))))
}
async fn get_decision(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let r=sqlx::query("SELECT d.*,p.name project_name,o.name confirmed_option_name FROM decisions d JOIN projects p ON p.id=d.project_id LEFT JOIN decision_options o ON o.id=d.confirmed_option_id WHERE d.id=?").bind(id).fetch_optional(&s.db).await?.ok_or(ApiError::NotFound)?;
    require_project_member(&s.db, &actor, r.get("project_id")).await?;
    let mut v = decision_json(&r);
    let metrics = sqlx::query("SELECT * FROM decision_metrics WHERE decision_id=? ORDER BY id")
        .bind(id)
        .fetch_all(&s.db)
        .await?;
    v["metrics"]=Value::Array(metrics.iter().map(|r|{let name:String=r.get("name");let key=metric_key(&name);json!({"id":r.get::<i64,_>("id"),"name":name,"key":key,"weight_bps":r.get::<i64,_>("weight_bps"),"weight":r.get::<i64,_>("weight_bps") as f64/100.0,"direction":r.get::<String,_>("direction"),"unit":r.get::<String,_>("unit")})}).collect());
    let options = sqlx::query("SELECT * FROM decision_options WHERE decision_id=? ORDER BY id")
        .bind(id)
        .fetch_all(&s.db)
        .await?;
    let mut option_values = Vec::new();
    for r in &options {
        let option_id: i64 = r.get("id");
        let values = sqlx::query(
            "SELECT metric_id,raw_value FROM decision_values WHERE option_id=? ORDER BY metric_id",
        )
        .bind(option_id)
        .fetch_all(&s.db)
        .await?;
        option_values.push(json!({"id":option_id,"name":r.get::<String,_>("name"),"description":r.get::<String,_>("description"),"initial_cost_cents":r.get::<i64,_>("initial_cost_cents"),"development_cost_cents":r.get::<i64,_>("development_cost_cents"),"annual_operation_cost_cents":r.get::<i64,_>("annual_operation_cost_cents"),"risk_probability_bps":r.get::<i64,_>("risk_probability_bps"),"risk_loss_cents":r.get::<i64,_>("risk_loss_cents"),"expected_benefit_cents":r.get::<i64,_>("expected_benefit_cents"),"values":values.iter().map(|x|json!({"metric_id":x.get::<i64,_>("metric_id"),"raw_value":x.get::<f64,_>("raw_value")})).collect::<Vec<_>>() }));
    }
    v["options"] = Value::Array(option_values);
    let results=sqlx::query("SELECT r.*,o.name FROM decision_results r JOIN decision_options o ON o.id=r.option_id WHERE r.decision_id=? ORDER BY rank").bind(id).fetch_all(&s.db).await?;
    v["results"]=Value::Array(results.iter().map(|r|json!({"option_id":r.get::<i64,_>("option_id"),"option_name":r.get::<String,_>("name"),"tco":cents_to_yuan(r.get("tco_cents")),"roi":r.get::<i64,_>("roi_bps") as f64/100.0,"total_score":r.get::<f64,_>("total_score"),"rank":r.get::<i64,_>("rank"),"scores":serde_json::from_str::<Value>(r.get("scores_json")).unwrap_or(json!({})),"advantages":serde_json::from_str::<Value>(r.get("advantages_json")).unwrap_or(json!([])),"disadvantages":serde_json::from_str::<Value>(r.get("disadvantages_json")).unwrap_or(json!([]))})).collect());
    Ok(Json(v))
}

#[derive(Debug)]
struct EvalOption {
    id: i64,
    name: String,
    tco: i64,
    roi: i64,
}
async fn evaluate_decision(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let decision = sqlx::query("SELECT analysis_years,status,project_id FROM decisions WHERE id=?")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    let years: i64 = decision.get("analysis_years");
    require_project_manager(&s.db, &actor, decision.get("project_id")).await?;
    if decision.get::<String, _>("status") == "confirmed" {
        return Err(ApiError::BadRequest("已确认的决策快照不能重新计算".into()));
    }
    let orows = sqlx::query("SELECT * FROM decision_options WHERE decision_id=? ORDER BY id")
        .bind(id)
        .fetch_all(&s.db)
        .await?;
    let mut opts = Vec::new();
    for r in orows {
        let risk =
            services::expected_risk_loss(r.get("risk_loss_cents"), r.get("risk_probability_bps"));
        let total = services::tco(
            r.get("initial_cost_cents"),
            r.get("development_cost_cents"),
            r.get("annual_operation_cost_cents"),
            years,
            risk,
        );
        opts.push(EvalOption {
            id: r.get("id"),
            name: r.get("name"),
            tco: total,
            roi: services::roi_bps(r.get("expected_benefit_cents"), total)?,
        });
    }
    let mrows = sqlx::query(
        "SELECT id,name,weight_bps,direction FROM decision_metrics WHERE decision_id=? ORDER BY id",
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    let mut scores = vec![0.0; opts.len()];
    let mut contributions = vec![Vec::<(String, String, f64, f64)>::new(); opts.len()];
    for m in &mrows {
        let mid: i64 = m.get("id");
        let metric_name: String = m.get("name");
        let is_economic =
            services::is_tco_metric(&metric_name) || services::is_roi_metric(&metric_name);
        let mut raw = Vec::new();
        for o in &opts {
            let supplied = if is_economic {
                0.0
            } else {
                sqlx::query_scalar::<_, f64>(
                    "SELECT raw_value FROM decision_values WHERE option_id=? AND metric_id=?",
                )
                .bind(o.id)
                .bind(mid)
                .fetch_optional(&s.db)
                .await?
                .ok_or_else(|| ApiError::BadRequest("方案指标数据不完整".into()))?
            };
            raw.push(services::resolve_decision_metric(
                &metric_name,
                supplied,
                o.tco,
                o.roi,
            ));
        }
        let normalized = services::normalize(&raw, m.get::<String, _>("direction") == "higher");
        let weight = m.get::<i64, _>("weight_bps") as f64 / 10000.0;
        let key = metric_key(&metric_name);
        for i in 0..opts.len() {
            let c = normalized[i] * weight;
            scores[i] += c;
            contributions[i].push((key.clone(), metric_name.clone(), normalized[i], c));
        }
    }
    let mut order: Vec<usize> = (0..opts.len()).collect();
    order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
    let mut result_json = Vec::new();
    let mut tx = s.db.begin().await?;
    sqlx::query("DELETE FROM decision_results WHERE decision_id=?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for (rank, &i) in order.iter().enumerate() {
        let mut c = contributions[i].clone();
        let score_map: BTreeMap<String, f64> = c
            .iter()
            .map(|(key, _, normalized, _)| (key.clone(), *normalized))
            .collect();
        c.sort_by(|a, b| b.3.total_cmp(&a.3));
        let advantages = c
            .iter()
            .take(2)
            .map(|(_, n, v, _)| format!("{n}表现突出（标准分{v:.1}）"))
            .collect::<Vec<_>>();
        c.sort_by(|a, b| a.2.total_cmp(&b.2));
        let disadvantages = c
            .iter()
            .take(1)
            .filter(|(_, _, v, _)| *v < 60.0)
            .map(|(_, n, v, _)| format!("{n}相对较弱（标准分{v:.1}）"))
            .collect::<Vec<_>>();
        sqlx::query("INSERT INTO decision_results(decision_id,option_id,tco_cents,roi_bps,total_score,rank,scores_json,advantages_json,disadvantages_json) VALUES(?,?,?,?,?,?,?,?,?)").bind(id).bind(opts[i].id).bind(opts[i].tco).bind(opts[i].roi).bind(scores[i]).bind(rank as i64+1).bind(serde_json::to_string(&score_map).unwrap()).bind(serde_json::to_string(&advantages).unwrap()).bind(serde_json::to_string(&disadvantages).unwrap()).execute(&mut *tx).await?;
        result_json.push(json!({"option_id":opts[i].id,"option_name":opts[i].name,"tco":cents_to_yuan(opts[i].tco),"roi":opts[i].roi as f64/100.0,"total_score":scores[i],"rank":rank+1,"scores":score_map,"advantages":advantages,"disadvantages":disadvantages}));
    }
    sqlx::query("UPDATE decisions SET status='evaluated' WHERE id=?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let top = &result_json[0];
    let reason = format!(
        "推荐{}：综合得分{:.1}，排名第一；TCO为{:.2}元，ROI为{:.2}% {}",
        top["option_name"].as_str().unwrap_or("该方案"),
        top["total_score"].as_f64().unwrap_or(0.0),
        top["tco"].as_f64().unwrap_or(0.0),
        top["roi"].as_f64().unwrap_or(0.0),
        top["advantages"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .unwrap_or("")
    );
    log(&s.db, actor.id, "decision", Some(id), "evaluate", &reason).await;
    Ok(Json(
        json!({"recommended_option_id":top["option_id"],"recommendation":reason,"results":result_json}),
    ))
}
async fn confirm_decision(
    State(s): State<AppState>,
    actor: Actor,
    Path(id): Path<i64>,
    Json(req): Json<ConfirmDecision>,
) -> ApiResult<Json<Value>> {
    nonempty(&req.reason, "确认理由")?;
    let decision = sqlx::query("SELECT status,project_id FROM decisions WHERE id=?")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    require_project_manager(&s.db, &actor, decision.get("project_id")).await?;
    let status: String = decision.get("status");
    if status != "evaluated" {
        return Err(ApiError::BadRequest(
            "决策必须完成评价且尚未确认才能确认方案".into(),
        ));
    }
    let valid: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM decision_options WHERE id=? AND decision_id=?")
            .bind(req.option_id)
            .bind(id)
            .fetch_one(&s.db)
            .await?;
    if valid == 0 {
        return Err(ApiError::BadRequest("候选方案不属于该决策".into()));
    }
    sqlx::query("UPDATE decisions SET status='confirmed',confirmed_option_id=?,confirmation_reason=?,confirmed_by=?,confirmed_at=CURRENT_TIMESTAMP WHERE id=?").bind(req.option_id).bind(&req.reason).bind(actor.id).bind(id).execute(&s.db).await?;
    log(
        &s.db,
        actor.id,
        "decision",
        Some(id),
        "confirm",
        &req.reason,
    )
    .await;
    Ok(Json(json!({"ok":true,"confirmed_option_id":req.option_id})))
}

#[derive(Deserialize)]
struct DashboardQuery {
    project_id: Option<i64>,
}
async fn dashboard(
    State(s): State<AppState>,
    actor: Actor,
    Query(q): Query<DashboardQuery>,
) -> ApiResult<Json<Value>> {
    let pid = match q.project_id {
        Some(v) => v,
        None if actor.role=="admin" => sqlx::query_scalar("SELECT id FROM projects WHERE status<>'archived' ORDER BY id DESC LIMIT 1").fetch_optional(&s.db).await?.ok_or(ApiError::NotFound)?,
        None => sqlx::query_scalar("SELECT p.id FROM projects p JOIN project_members pm ON pm.project_id=p.id WHERE p.status<>'archived' AND pm.user_id=? ORDER BY p.id DESC LIMIT 1").bind(actor.id).fetch_optional(&s.db).await?.ok_or(ApiError::NotFound)?,
    };
    require_project_member(&s.db, &actor, pid).await?;
    let p=sqlx::query("SELECT p.*,(SELECT COALESCE(AVG(progress),0.0) FROM tasks WHERE project_id=p.id) completion FROM projects p WHERE id=?").bind(pid).fetch_optional(&s.db).await?.ok_or(ApiError::NotFound)?;
    let status_rows =
        sqlx::query("SELECT status,COUNT(*) count FROM tasks WHERE project_id=? GROUP BY status")
            .bind(pid)
            .fetch_all(&s.db)
            .await?;
    let mut status_counts = json!({"todo":0,"in_progress":0,"review":0,"testing":0,"done":0});
    for r in status_rows {
        status_counts[r.get::<String, _>("status")] = json!(r.get::<i64, _>("count"));
    }
    let (budget, labor, expense) = cost_values(&s.db, pid).await?;
    let evm = evm_for(&s.db, pid).await?;
    let risks = risk_values(&s.db, pid).await?;
    let milestones=sqlx::query("SELECT id,name,end_date due_date,status FROM milestones WHERE project_id=? AND status<>'completed' ORDER BY end_date LIMIT 5").bind(pid).fetch_all(&s.db).await?;
    let member_rows=sqlx::query("SELECT u.id,u.display_name name,pm.project_role role,u.hourly_rate_cents,COALESCE(tm.weekly_capacity_hours,40.0) capacity,COALESCE(SUM(t.estimated_hours),0.0) assigned_hours,COUNT(t.id) active_tasks FROM project_members pm JOIN users u ON u.id=pm.user_id JOIN projects p ON p.id=pm.project_id LEFT JOIN team_members tm ON tm.team_id=p.team_id AND tm.user_id=u.id LEFT JOIN tasks t ON t.project_id=pm.project_id AND t.assignee_id=u.id AND t.status<>'done' WHERE pm.project_id=? GROUP BY u.id,u.display_name,pm.project_role,u.hourly_rate_cents,tm.weekly_capacity_hours ORDER BY assigned_hours DESC").bind(pid).fetch_all(&s.db).await?;
    let member_loads=member_rows.iter().map(|r|{let assigned:f64=r.get("assigned_hours");let capacity:f64=r.get("capacity");json!({"id":r.get::<i64,_>("id"),"name":r.get::<String,_>("name"),"role":r.get::<String,_>("role"),"hourly_rate":cents_to_yuan(r.get("hourly_rate_cents")),"assigned_hours":assigned,"available_hours":capacity,"capacity":capacity,"load_rate":if capacity>0.0{assigned/capacity*100.0}else{0.0},"active_tasks":r.get::<i64,_>("active_tasks")})}).collect::<Vec<_>>();
    let trend_rows=sqlx::query("SELECT cost_date,SUM(value_cents) value_cents FROM (SELECT w.work_date cost_date,w.hours*u.hourly_rate_cents value_cents FROM worklogs w JOIN tasks t ON t.id=w.task_id JOIN users u ON u.id=w.user_id WHERE t.project_id=? UNION ALL SELECT occurred_on cost_date,CAST(amount_cents AS REAL) value_cents FROM expenses WHERE project_id=?) GROUP BY cost_date ORDER BY cost_date").bind(pid).bind(pid).fetch_all(&s.db).await?;
    let mut cumulative_cents = 0.0;
    let mut cost_trend = Vec::new();
    for r in trend_rows {
        cumulative_cents += r.get::<f64, _>("value_cents");
        cost_trend
            .push(json!({"date":r.get::<String,_>("cost_date"),"value":cumulative_cents/100.0}));
    }
    let overdue_task_count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE project_id=? AND status<>'done' AND planned_end<date('now')").bind(pid).fetch_one(&s.db).await?;
    Ok(Json(
        json!({"project_id":pid,"project_name":p.get::<String,_>("name"),"completion_rate":p.get::<f64,_>("completion"),"task_counts":status_counts,"budget":cents_to_yuan(budget),"actual_cost":cents_to_yuan(labor+expense),"budget_usage_rate":if budget>0{(labor+expense)as f64/budget as f64*100.0}else{0.0},"evm":evm,"risk_count":risks.len(),"high_risk_count":risks.iter().filter(|v|v["level"]=="high").count(),"overdue_task_count":overdue_task_count,"latest_risks":risks.into_iter().take(5).collect::<Vec<_>>(),"upcoming_milestones":milestones.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"name":r.get::<String,_>("name"),"due_date":r.get::<String,_>("due_date"),"status":r.get::<String,_>("status")})).collect::<Vec<_>>(),"member_loads":member_loads,"cost_trend":cost_trend }),
    ))
}

#[derive(Deserialize)]
struct LogQuery {
    entity_type: Option<String>,
    entity_id: Option<i64>,
}
async fn activity_logs(
    State(s): State<AppState>,
    actor: Actor,
    Query(q): Query<LogQuery>,
) -> ApiResult<Json<Value>> {
    auth::require(&actor, &["admin"])?;
    let rows=sqlx::query("SELECT l.*,u.display_name actor_name FROM activity_logs l LEFT JOIN users u ON u.id=l.actor_id WHERE (? IS NULL OR entity_type=?) AND (? IS NULL OR entity_id=?) ORDER BY l.id DESC LIMIT 200").bind(&q.entity_type).bind(&q.entity_type).bind(q.entity_id).bind(q.entity_id).fetch_all(&s.db).await?;
    Ok(Json(Value::Array(rows.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"actor_id":r.get::<Option<i64>,_>("actor_id"),"actor_name":r.get::<Option<String>,_>("actor_name"),"entity_type":r.get::<String,_>("entity_type"),"entity_id":r.get::<Option<i64>,_>("entity_id"),"action":r.get::<String,_>("action"),"detail":r.get::<String,_>("detail"),"created_at":r.get::<String,_>("created_at")})).collect())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn seeded_app() -> Router {
        let pool = crate::db::connect("sqlite::memory:").await.unwrap();
        router(AppState {
            db: pool,
            jwt_secret: "test-secret".into(),
        })
    }

    async fn login_as(app: &Router, username: &str) -> String {
        let body = json!({"username":username,"password":"RustFlow123!"}).to_string();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice::<Value>(&bytes).unwrap()["token"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn authed(
        app: &Router,
        method: &str,
        uri: &str,
        token: &str,
        body: Value,
    ) -> axum::response::Response {
        app.clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn health_endpoint_works() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let app = router(AppState {
            db: pool,
            jwt_secret: "test".into(),
        });
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap()["status"],
            "ok"
        );
    }

    #[tokio::test]
    async fn login_returns_jwt_and_user() {
        use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
        use rand_core::OsRng;
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE users(id INTEGER PRIMARY KEY,username TEXT,password_hash TEXT,display_name TEXT,role TEXT,hourly_rate_cents INTEGER,active INTEGER)")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE activity_logs(id INTEGER PRIMARY KEY,actor_id INTEGER,entity_type TEXT,entity_id INTEGER,action TEXT,detail TEXT,created_at TEXT DEFAULT CURRENT_TIMESTAMP)")
            .execute(&pool).await.unwrap();
        let hash = Argon2::default()
            .hash_password(b"secret", &SaltString::generate(&mut OsRng))
            .unwrap()
            .to_string();
        sqlx::query("INSERT INTO users VALUES(1,'tester',?,'测试用户','developer',8000,1)")
            .bind(hash)
            .execute(&pool)
            .await
            .unwrap();
        let app = router(AppState {
            db: pool,
            jwt_secret: "test-secret".into(),
        });
        let req = Request::builder()
            .method("POST")
            .uri("/api/auth/login")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"username":"tester","password":"secret"}"#))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert!(value["token"].as_str().is_some());
        assert_eq!(value["user"]["role"], "developer");
    }

    #[tokio::test]
    async fn project_roles_and_relations_are_enforced() {
        let app = seeded_app().await;
        let dev = login_as(&app, "dev").await;
        let forbidden = authed(&app, "PUT", "/api/tasks/1", &dev, json!({"progress":50})).await;
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

        let manager = login_as(&app, "manager").await;
        let outsider = authed(
            &app,
            "POST",
            "/api/projects/1/members",
            &manager,
            json!({"user_id":1,"project_role":"developer"}),
        )
        .await;
        assert_eq!(outsider.status(), StatusCode::BAD_REQUEST);
        let invalid_task=authed(&app,"POST","/api/projects/1/tasks",&manager,json!({"milestone_id":999,"title":"非法关系","description":"","acceptance_criteria":"","assignee_id":3,"participant_ids":[],"priority":"medium","planned_start":"2026-09-01","planned_end":"2026-09-02","estimated_hours":1})).await;
        assert_eq!(invalid_task.status(), StatusCode::BAD_REQUEST);
        let invalid_priority=authed(&app,"POST","/api/projects/1/tasks",&manager,json!({"milestone_id":2,"title":"非法优先级","description":"","acceptance_criteria":"","assignee_id":3,"participant_ids":[],"priority":"impossible","planned_start":"2026-09-01","planned_end":"2026-09-02","estimated_hours":1})).await;
        assert_eq!(invalid_priority.status(), StatusCode::BAD_REQUEST);
        let invalid_progress = authed(
            &app,
            "PUT",
            "/api/tasks/1",
            &manager,
            json!({"progress":101}),
        )
        .await;
        assert_eq!(invalid_progress.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn team_management_and_decision_inputs_are_validated() {
        let app = seeded_app().await;
        let dev = login_as(&app, "dev").await;
        let denied = authed(
            &app,
            "POST",
            "/api/teams/1/members",
            &dev,
            json!({"user_id":4,"role":"reviewer","weekly_capacity_hours":40}),
        )
        .await;
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        let manager = login_as(&app, "manager").await;
        let capacity = authed(
            &app,
            "POST",
            "/api/teams/1/members",
            &manager,
            json!({"user_id":4,"role":"reviewer","weekly_capacity_hours":0}),
        )
        .await;
        assert_eq!(capacity.status(), StatusCode::BAD_REQUEST);
        let demote = authed(
            &app,
            "POST",
            "/api/teams/1/members",
            &manager,
            json!({"user_id":2,"role":"developer","weekly_capacity_hours":40}),
        )
        .await;
        assert_eq!(demote.status(), StatusCode::OK);
        let create_project=authed(&app,"POST","/api/projects",&manager,json!({"team_id":1,"name":"无团队经理权限","description":"","manager_id":2,"start_date":"2026-09-01","end_date":"2026-10-01","budget_cents":10000})).await;
        assert_eq!(create_project.status(), StatusCode::FORBIDDEN);
        let invalid_decision=authed(&app,"POST","/api/decisions",&manager,json!({"project_id":1,"title":"","description":"","analysis_years":0,"metrics":[{"name":"TCO","weight_bps":10000,"direction":"lower","unit":"元"}],"options":[{"name":"A","description":"","initial_cost_cents":0,"development_cost_cents":0,"annual_operation_cost_cents":0,"risk_probability_bps":0,"risk_loss_cents":0,"expected_benefit_cents":0,"values":[0]},{"name":"B","description":"","initial_cost_cents":0,"development_cost_cents":0,"annual_operation_cost_cents":0,"risk_probability_bps":0,"risk_loss_cents":0,"expected_benefit_cents":0,"values":[0]}]})).await;
        assert_eq!(invalid_decision.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn archived_project_rejects_business_writes() {
        let app = seeded_app().await;
        let manager = login_as(&app, "manager").await;
        let archived = authed(
            &app,
            "PUT",
            "/api/projects/1",
            &manager,
            json!({"status":"archived"}),
        )
        .await;
        assert_eq!(archived.status(), StatusCode::OK);
        let write=authed(&app,"POST","/api/projects/1/milestones",&manager,json!({"name":"不应创建","description":"","start_date":"2026-10-01","end_date":"2026-10-02"})).await;
        assert_eq!(write.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn decision_get_returns_normalized_score_dictionary() {
        let app = seeded_app().await;
        let manager = login_as(&app, "manager").await;
        let evaluated = authed(
            &app,
            "POST",
            "/api/decisions/1/evaluate",
            &manager,
            json!({}),
        )
        .await;
        assert_eq!(evaluated.status(), StatusCode::OK);
        let response = authed(&app, "GET", "/api/decisions/1", &manager, json!({})).await;
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        let scores = value["results"][0]["scores"].as_object().unwrap();
        assert!(scores.contains_key("feasibility"));
        assert!(scores.contains_key("security"));
    }
}
