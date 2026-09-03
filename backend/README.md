# RustFlow Backend

RustFlow 的 Rust/Axum REST 后端。SQLite 内部金额统一使用整数分；写入请求使用 `*_cents`，面向页面的项目、支出、财务和决策响应按各 DTO 提供元单位展示字段，EVM 同时保留 `*_cents`。

## 启动

```powershell
Copy-Item .env.example .env
cargo run
```

默认地址：`http://127.0.0.1:3000`。首次启动会自动创建 `rustflow.db`、执行迁移并写入演示数据。

演示账号密码均为 `RustFlow123!`：`admin`、`manager`、`dev`、`reviewer`。

## 测试

```powershell
cargo test
```

业务接口统一以 `/api` 开头，除 `/health`、`/api/health` 和 `/api/auth/login` 外均需 `Authorization: Bearer <token>`。下列路径均省略 `/api` 前缀。

主要接口：

- 认证与用户：`/auth/login`、`/auth/me`、`/auth/logout`、`/users`
- 团队与项目：`/teams`、`/teams/{id}/members`、`/projects`、`/projects/{id}`、`/projects/{id}/members`、`/projects/{id}/milestones`
- 研发协作：`/projects/{id}/tasks`、`/tasks/{id}`、`/tasks/{id}/transition`、`/tasks/{id}/review`、`/tasks/{id}/comments`、`/tasks/{id}/dependencies`
- 工时与财务（前端主路径）：`/projects/{id}/worklogs`、`/projects/{id}/expenses`、`/projects/{id}/finance/summary`
- 风险与决策（前端主路径）：`/projects/{id}/risks`、`/risks/scan`、`/decisions`、`/decisions/{id}`、`/decisions/{id}/evaluate`、`/decisions/{id}/confirm`
- 汇总与审计：`/dashboard?project_id=1`、`/activity-logs`

为便于其他客户端接入，后端还保留等价的全局兼容路径：`/worklogs?project_id=1`、`/expenses?project_id=1`、`/finance/summary?project_id=1`、`/risks?project_id=1`；项目财务汇总另有 `/projects/{id}/metrics` 别名。

列表响应为裸 JSON 数组，错误响应为 `{"error":"错误说明"}`。金额写入字段使用整数分，概率和决策权重使用整数基点；完整请求体、权限和响应字段见 [`../docs/architecture-and-data-model.md`](../docs/architecture-and-data-model.md)。
