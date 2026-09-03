# 系统架构、数据模型与 API 契约

## 1. 总体架构

RustFlow 采用浏览器端、Rust API 服务和 SQLite 三层结构。前端负责表单、看板和图表，不保存可信业务结论；权限、流程、图算法和经济计算均以后端返回结果为准。

```text
┌──────────────────────────────────────────┐
│ Vue 3 / TypeScript / Element Plus        │
│ 页面、表单、看板、ECharts、登录状态       │
└─────────────────────┬────────────────────┘
                      │ REST / JSON / JWT
┌─────────────────────▼────────────────────┐
│ Rust / Axum / Tokio / Serde              │
│ RBAC │ task state │ dependency │ EVM     │
│ cost │ risk │ TCO/ROI │ weighted score  │
└─────────────────────┬────────────────────┘
                      │ SQLx / transaction
┌─────────────────────▼────────────────────┐
│ SQLite                                   │
│ 业务数据、评价结果、审计记录              │
└──────────────────────────────────────────┘
```

默认 API 地址为 `http://127.0.0.1:3000`，业务接口前缀为 `/api`；前端开发服务器通过 Vite 将 `/api` 代理到后端。

## 2. 核心数据模型

| 实体 | 关键字段 | 实现约束 |
| --- | --- | --- |
| User | username, password_hash, display_name, role, hourly_rate_cents, active | username 唯一；系统角色为 admin/manager/developer/reviewer |
| Team | name, description, owner_id | 创建者自动成为团队 manager |
| TeamMember | team_id, user_id, role, weekly_capacity_hours | 决定团队成员关系和负载分母 |
| Project | team_id, manager_id, dates, budget_cents, status | status 为 planning/active/completed/archived |
| ProjectMember | project_id, user_id, project_role | 项目角色为 manager/developer/reviewer |
| Milestone | project_id, name, dates, status | 项目内阶段节点 |
| Task | project_id, milestone_id?, assignee_id?, priority, planned dates, estimated_hours, progress, status | 状态机由 Rust 校验 |
| TaskParticipant | task_id, user_id | 开发协作者 |
| TaskDependency | task_id, depends_on_task_id | 同项目、不可自环、不可成环、组合唯一 |
| TaskComment/TaskReview | author/reviewer, content/result/stage, created_at | 驳回意见必填 |
| Worklog | task_id, user_id, work_date, hours, content | 只能由有贡献权限的成员登记本人记录 |
| Expense | project_id, category, amount_cents, occurred_on, description | category 为 device/cloud/purchase/other |
| RiskAlert | project_id, task_id?, kind, level, message, resolved | 手动扫描时更新当前活动预警 |
| Decision | project_id, title, analysis_years, status, confirmed_option_id?, confirmation_reason? | draft/evaluated/confirmed |
| DecisionOption | 成本、风险、收益输入 | 金额存整数分，概率存整数基点 |
| DecisionMetric/DecisionValue | name, key, weight_bps, direction, raw_value | 权重合计 10000；方向 higher/lower |
| DecisionResult | option_id, tco_cents, roi_bps, total_score, rank, scores_json, advantages_json, disadvantages_json | 保存服务端评价结果和各指标标准分 |
| ActivityLog | actor_id, entity_type, entity_id, action, detail, created_at | 记录关键写操作；全局查询限管理员 |

金额输入和数据库结算使用整数分（`*_cents`），概率与权重使用整数基点（`*_bps`，10000 表示 100%）。面向页面的部分响应同时提供换算后的元、百分比或浮点指标，前端不得据此反向写回金额。

## 3. 核心业务规则

### 3.1 角色与项目边界

- 系统角色决定用户管理、团队/项目创建等全局能力。
- 团队成员变更仅允许系统管理员、团队所有者或团队 manager；非管理员只有以团队 manager 身份才能在该团队创建项目。
- 项目角色决定项目内写权限；系统管理员访问项目时视为项目管理员。
- 非管理员只能列出自己加入的项目和决策，并且项目详情、任务、财务、风险等查询都校验项目成员关系。
- 项目经理维护项目、成员、里程碑、任务结构、依赖、支出、风险扫描和技术决策。
- 项目 developer 仅能修改本人负责或参与任务的内容/进度、发表评论、流转开发阶段并登记工时，不能修改负责人、参与者、里程碑、计划日期或预计工时。
- 项目 reviewer 执行评审和测试结论；manager/admin 也可执行。
- 用户须先加入项目所属团队，才能加入项目；项目负责人也必须属于该团队。

项目更新使用 `PUT /api/projects/{id}`。提交 `{"status":"archived"}` 后项目进入只读状态；保留查询能力，但所有经项目写权限保护的新增、修改、流转、评价和确认操作均被拒绝。

### 3.2 任务状态机与依赖图

```text
todo → in_progress → review → testing → done
                        │          │
                        └─ reject ─┴─→ in_progress
```

- `todo → in_progress`、`in_progress → review` 由项目 developer（且为负责人/参与者）、manager 或 admin 执行。
- `review → testing`、`testing → done` 以及两个阶段的驳回由 reviewer、manager 或 admin 执行。
- 评审请求的 result 只能为 `approved` 或 `rejected`；驳回时 comment 必填。
- 前置任务未完成时不能进入 `in_progress`；进入 `done` 时完成比例置为 100。
- 新增依赖时校验同一项目、自依赖和环路。下游接口从指定任务沿反向依赖关系广度遍历，返回全部受影响任务 ID。

### 3.3 成本、负载、EVM 与风险

- `labor_cost = Σ(worklog.hours × user.hourly_rate_cents)`
- `actual_cost = labor_cost + Σ(expense.amount_cents)`
- 成员负载率为未完成任务预计工时合计除以团队周可用工时；大于 100% 产生过载风险。
- 项目预算按任务预计工时比例分摊。PV 再按当前日期处于任务计划周期的位置计算，EV 按任务完成比例计算，AC 为实际人力和分类支出之和，BAC 为项目预算。
- `SPI = EV / PV`，`CPI = EV / AC`，`EAC = BAC / CPI`；无有效分母时对应值为 `null`。
- 风险由当前数据实时计算，类型包括 overdue、dependency_blocked、overload、budget_overrun、forecast_overrun。逾期风险同时返回 `affected_task_ids` 和带任务名称、负责人的 `affected_tasks`；`POST /api/risks/scan` 还会覆盖保存当前未解决风险记录。

### 3.4 技术方案评价

- 预期风险损失 = 风险损失金额 × 风险概率。
- TCO = 初始投入 + 开发成本 + 年运维成本 × 分析年限 + 预期风险损失。
- ROI =（预期收益 − TCO）/ TCO × 100%；TCO 不大于 0 时拒绝评价。
- higher 指标使用 `(value-min)/(max-min)×100`，lower 指标使用 `(max-value)/(max-min)×100`；一列数值相同时统一记 100 分。
- 综合得分为各指标标准分乘权重之和，并按综合得分降序排名。
- 名称为 TCO/总拥有成本或 ROI/投资回报率的指标，其原始值由 Rust 根据方案经济数据覆盖客户端占位值。
- 每个结果保存 `scores`（指标 key 到 0～100 标准分的映射）、优势、不足、TCO、ROI、总分和名次，雷达图直接使用 `scores`。
- 决策须先评价再人工确认。确认可选择非推荐方案，但理由必填；确认后输入与已保存结果共同形成只读记录，不能再次评价。

## 4. REST API 契约

### 4.1 通用规则

- 请求与响应为 `application/json`。
- 除健康检查和登录外，请求头为 `Authorization: Bearer <token>`。
- 日期使用 `YYYY-MM-DD`；数据库时间戳按 SQLite 格式返回。
- 列表直接返回 JSON 数组，例如 `[{...}]`，没有 `items/total` 包装。
- 创建操作通常返回 `201 {"id": 1}`；更新或动作接口通常返回 `{"ok": true}` 及动作字段。
- 错误体统一为 `{"error":"错误说明"}`；当前使用 400、401、403、404 和 500 状态码。

### 4.2 已实现端点

| 方法与路径 | 用途/关键请求体 |
| --- | --- |
| GET `/health`、GET `/api/health` | 健康检查，返回 `{"status":"ok"}` |
| POST `/api/auth/login` | `{username,password}`，返回 `{token,user}` |
| GET `/api/auth/me`、POST `/api/auth/logout` | 当前用户、退出审计 |
| GET/POST `/api/users`、PUT `/api/users/{id}` | 用户列表/创建/更新 |
| GET/POST `/api/teams` | 团队列表/创建 |
| GET/POST `/api/teams/{id}/members` | 团队成员列表/新增或更新成员 |
| GET/POST `/api/projects`、GET/PUT `/api/projects/{id}` | 项目列表/创建/详情/更新与归档 |
| GET/POST `/api/projects/{id}/members` | 项目成员列表/新增或更新项目角色 |
| GET/POST `/api/projects/{id}/milestones` | 里程碑列表/创建 |
| GET/POST `/api/projects/{id}/tasks`、GET/PUT `/api/tasks/{id}` | 任务列表/创建/详情/更新 |
| GET/POST `/api/tasks/{id}/comments` | 评论列表/新增，body `{content}` |
| POST `/api/tasks/{id}/transition` | 状态流转，body `{to_status}` |
| POST `/api/tasks/{id}/review` | 评审/测试，body `{result,comment}` |
| GET/POST `/api/tasks/{id}/dependencies` | 前置任务列表/新增，body `{depends_on_task_id}` |
| DELETE `/api/tasks/{id}/dependencies/{dependency_id}` | 删除前置关系 |
| GET `/api/tasks/{id}/downstream` | `{task_id,affected_task_ids,affected_count}` |
| GET/POST `/api/projects/{id}/worklogs` | 项目工时列表/新增，body `{task_id,work_date,hours,content}` |
| GET/POST `/api/projects/{id}/expenses` | 项目支出列表/新增，body `{category,amount_cents,occurred_on,description}` |
| GET `/api/projects/{id}/finance/summary` | 成本分类、预算差异和 EVM |
| GET `/api/projects/{id}/metrics` | 财务汇总的兼容别名 |
| GET `/api/projects/{id}/risks` | 实时风险数组 |
| GET `/api/finance/summary?project_id={id}` | 财务汇总的查询参数形式 |
| GET/POST `/api/worklogs?project_id={id}` | 工时的查询参数形式 |
| GET/POST `/api/expenses?project_id={id}` | 支出的查询参数形式 |
| GET `/api/risks?project_id={id}` | 风险的查询参数形式 |
| POST `/api/risks/scan` | 重新扫描并保存活动风险，body `{project_id}` |
| GET/POST `/api/decisions` | 有权决策列表/聚合创建 |
| GET `/api/decisions/{id}` | 决策、指标、方案、评价结果和人工确认详情 |
| POST `/api/decisions/{id}/evaluate` | Rust 评价并覆盖该决策旧结果 |
| POST `/api/decisions/{id}/confirm` | body `{option_id,reason}`；确认后不可重算 |
| GET `/api/dashboard?project_id={id}` | 项目综合仪表盘 |
| GET `/api/activity-logs?entity_type=&entity_id=` | 管理员查询操作日志 |

项目嵌套的工时、支出、财务和风险路径是前端当前使用的主路径；全局查询参数形式是兼容接口。

### 4.3 关键请求与响应

状态流转：

```json
POST /api/tasks/2/transition
{
  "to_status": "review"
}
```

评审/测试：

```json
POST /api/tasks/2/review
{
  "result": "rejected",
  "comment": "验收标准第 2 项未满足"
}
```

创建决策时一次提交指标和候选方案。`values` 数组必须与 `metrics` 顺序、数量一致；金额为分，权重和概率为基点：

```json
{
  "project_id": 1,
  "title": "部署方式选择",
  "description": "比较三年期部署方案",
  "analysis_years": 3,
  "metrics": [
    {"name":"TCO","weight_bps":3000,"direction":"lower","unit":"元"},
    {"name":"ROI","weight_bps":2000,"direction":"higher","unit":"%"},
    {"name":"安全性","weight_bps":5000,"direction":"higher","unit":"分"}
  ],
  "options": [
    {
      "name":"公有云",
      "description":"按需扩展",
      "initial_cost_cents":1000000,
      "development_cost_cents":2000000,
      "annual_operation_cost_cents":800000,
      "risk_probability_bps":1000,
      "risk_loss_cents":500000,
      "expected_benefit_cents":8000000,
      "values":[0,0,92]
    },
    {
      "name":"自建服务器",
      "description":"自主管理",
      "initial_cost_cents":3000000,
      "development_cost_cents":2200000,
      "annual_operation_cost_cents":600000,
      "risk_probability_bps":800,
      "risk_loss_cents":600000,
      "expected_benefit_cents":20000000,
      "values":[0,0,85]
    }
  ]
}
```

评价响应：

```json
{
  "recommended_option_id": 1,
  "recommendation": "推荐公有云：综合得分……",
  "results": [
    {
      "option_id": 1,
      "option_name": "公有云",
      "tco": 54500.0,
      "roi": 46.79,
      "total_score": 80.0,
      "rank": 1,
      "scores": {"tco":100.0,"roi":0.0,"security":100.0},
      "advantages": ["TCO表现突出（标准分100.0）"],
      "disadvantages": ["ROI相对较弱（标准分0.0）"]
    }
  ]
}
```

仪表盘响应的稳定字段为：`project_id`、`project_name`、`completion_rate`、`task_counts`、`budget`、`actual_cost`、`budget_usage_rate`、`evm`、`risk_count`、`high_risk_count`、`overdue_task_count`、`latest_risks`、`upcoming_milestones`、`member_loads`、`cost_trend`。其中：

- `task_counts` 始终包含 todo、in_progress、review、testing、done；
- `evm` 包含 pv_cents、ev_cents、ac_cents、bac_cents、spi、cpi、eac_cents、health；
- `cost_trend` 为按日期累计的 `{date,value}` 元金额序列；
- `member_loads` 包含 assigned_hours、available_hours、load_rate、active_tasks。

## 5. 一致性与失败处理

- 项目创建、任务创建/参与者更新、评审、风险扫描和决策创建/评价使用事务保护相关多表写入。
- 数据库唯一约束避免重复成员和重复依赖；依赖接口对重复关系幂等处理。
- 当前没有项目或任务删除 API。归档是项目生命周期状态，不物理删除历史数据。
- 前端显示后端 `error` 字段；关键业务动作成功后再刷新界面，不以前端状态替代服务端结论。

## 6. 第一版边界

本文只描述仓库当前基础版。第一版不包含人员推荐、研发设备借还与冲突检测、决策敏感性分析、报告导出、大模型/第三方 AI、考勤请假、即时聊天、在线文档、视频会议、移动端和复杂多租户。
