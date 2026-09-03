use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use rand_core::OsRng;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::str::FromStr;

pub async fn connect(url: &str) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(if url.contains(":memory:") { 1 } else { 8 })
        .connect_with(options)
        .await?;
    sqlx::migrate!().run(&pool).await?;
    seed(&pool).await?;
    Ok(pool)
}

async fn seed(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(());
    }
    let hash = Argon2::default()
        .hash_password(b"RustFlow123!", &SaltString::generate(&mut OsRng))
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .to_string();
    let users = [
        ("admin", "系统管理员", "admin", 12000),
        ("manager", "项目经理", "manager", 10000),
        ("dev", "开发工程师", "developer", 8000),
        ("reviewer", "测试评审员", "reviewer", 7000),
    ];
    let mut tx = pool.begin().await?;
    for (username, name, role, rate) in users {
        sqlx::query("INSERT INTO users(username,password_hash,display_name,role,hourly_rate_cents) VALUES(?,?,?,?,?)")
            .bind(username).bind(&hash).bind(name).bind(role).bind(rate).execute(&mut *tx).await?;
    }
    sqlx::query(
        "INSERT INTO teams(name,description,owner_id) VALUES('RustFlow研发组','课程演示团队',2)",
    )
    .execute(&mut *tx)
    .await?;
    for (id, role) in [(2, "manager"), (3, "developer"), (4, "reviewer")] {
        sqlx::query("INSERT INTO team_members(team_id,user_id,role) VALUES(1,?,?)")
            .bind(id)
            .bind(role)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("INSERT INTO projects(team_id,name,description,manager_id,start_date,end_date,budget_cents,status) VALUES(1,'企业知识库系统','RustFlow完整演示项目',2,'2026-08-01','2026-10-31',10000000,'active')").execute(&mut *tx).await?;
    for (id, role) in [(2, "manager"), (3, "developer"), (4, "reviewer")] {
        sqlx::query("INSERT INTO project_members(project_id,user_id,project_role) VALUES(1,?,?)")
            .bind(id)
            .bind(role)
            .execute(&mut *tx)
            .await?;
    }
    for (name, start, end, status) in [
        ("需求设计", "2026-08-01", "2026-08-15", "completed"),
        ("系统开发", "2026-08-16", "2026-09-30", "active"),
        ("联调测试", "2026-10-01", "2026-10-20", "pending"),
        ("项目交付", "2026-10-21", "2026-10-31", "pending"),
    ] {
        sqlx::query(
            "INSERT INTO milestones(project_id,name,start_date,end_date,status) VALUES(1,?,?,?,?)",
        )
        .bind(name)
        .bind(start)
        .bind(end)
        .bind(status)
        .execute(&mut *tx)
        .await?;
    }
    let task_sql = "INSERT INTO tasks(project_id,milestone_id,title,description,acceptance_criteria,assignee_id,priority,status,planned_start,planned_end,estimated_hours,progress) VALUES(1,?,?,?,?,?,?,?,?,?,?,?)";
    sqlx::query(task_sql)
        .bind(1)
        .bind("需求确认")
        .bind("完成需求评审")
        .bind("评审通过")
        .bind(2)
        .bind("high")
        .bind("done")
        .bind("2026-08-01")
        .bind("2026-08-12")
        .bind(20.0)
        .bind(100)
        .execute(&mut *tx)
        .await?;
    sqlx::query(task_sql)
        .bind(2)
        .bind("Rust后端接口")
        .bind("完成核心REST接口")
        .bind("测试通过")
        .bind(3)
        .bind("critical")
        .bind("in_progress")
        .bind("2026-08-16")
        .bind("2026-09-08")
        .bind(80.0)
        .bind(65)
        .execute(&mut *tx)
        .await?;
    sqlx::query(task_sql)
        .bind(3)
        .bind("系统联调")
        .bind("前后端联调")
        .bind("关键流程可运行")
        .bind(3)
        .bind("high")
        .bind("todo")
        .bind("2026-09-25")
        .bind("2026-10-10")
        .bind(40.0)
        .bind(0)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO task_dependencies(task_id,depends_on_task_id) VALUES(2,1),(3,2)")
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO worklogs(task_id,user_id,work_date,hours,content) VALUES(2,3,'2026-09-01',8,'实现认证与项目接口'),(2,3,'2026-09-02',7,'实现任务状态机')").execute(&mut *tx).await?;
    sqlx::query("INSERT INTO expenses(project_id,category,amount_cents,occurred_on,description,created_by) VALUES(1,'cloud',120000,'2026-09-01','云测试环境',2)").execute(&mut *tx).await?;
    sqlx::query("INSERT INTO task_comments(task_id,author_id,content) VALUES(2,2,'请优先完成权限与任务状态接口')").execute(&mut *tx).await?;
    let did=sqlx::query("INSERT INTO decisions(project_id,title,description,analysis_years,created_by) VALUES(1,'部署方案选择','比较三年期部署方案',3,2)").execute(&mut *tx).await?.last_insert_rowid();
    let mut metric_ids = Vec::new();
    for (name, weight, direction, unit) in [
        ("技术可行性", 3000, "higher", "分"),
        ("安全性", 2500, "higher", "分"),
        ("扩展能力", 2000, "higher", "分"),
        ("开发周期", 2500, "lower", "天"),
    ] {
        metric_ids.push(sqlx::query("INSERT INTO decision_metrics(decision_id,name,weight_bps,direction,unit) VALUES(?,?,?,?,?)").bind(did).bind(name).bind(weight).bind(direction).bind(unit).execute(&mut *tx).await?.last_insert_rowid());
    }
    let demo_options = [
        (
            "自建服务器",
            2_000_000,
            600_000,
            800_000,
            2000,
            1_000_000,
            7_000_000,
            [85.0, 95.0, 60.0, 90.0],
        ),
        (
            "公有云",
            300_000,
            300_000,
            1_200_000,
            1000,
            800_000,
            7_000_000,
            [90.0, 80.0, 95.0, 30.0],
        ),
        (
            "混合云",
            1_000_000,
            500_000,
            900_000,
            1500,
            900_000,
            7_000_000,
            [88.0, 90.0, 85.0, 60.0],
        ),
    ];
    for (name, initial, development, ops, probability, loss, benefit, values) in demo_options {
        let oid=sqlx::query("INSERT INTO decision_options(decision_id,name,initial_cost_cents,development_cost_cents,annual_operation_cost_cents,risk_probability_bps,risk_loss_cents,expected_benefit_cents) VALUES(?,?,?,?,?,?,?,?)").bind(did).bind(name).bind(initial).bind(development).bind(ops).bind(probability).bind(loss).bind(benefit).execute(&mut *tx).await?.last_insert_rowid();
        for (mid, value) in metric_ids.iter().zip(values) {
            sqlx::query("INSERT INTO decision_values(option_id,metric_id,raw_value) VALUES(?,?,?)")
                .bind(oid)
                .bind(mid)
                .bind(value)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}
