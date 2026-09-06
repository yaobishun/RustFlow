# RustFlow：基于 Rust 的研发团队协同与工程决策支持系统

RustFlow 是“基于 Rust 的研发团队协同与工程决策支持系统的设计与实现”课程实践项目，面向企业内部的中小型研发团队。系统以研发任务协作为主线，将项目计划、任务评审、工时成本、挣值管理（EVM）、依赖风险传播和技术方案经济评价组织在同一套工作流中，为项目负责人提供进度分析、成本预警和技术方案决策支持。

项目仓库：<https://github.com/yaobishun/RustFlow>

> 本仓库交付的是可落地的基础版，不包含人员智能推荐、研发设备管理、决策敏感性分析、报告导出、大模型助手、考勤、请假、即时聊天和在线文档。

## 核心业务闭环

```text
建立团队
  → 创建项目、预算和里程碑
  → 分解并分配研发任务
  → 开发、评审和测试协作
  → 记录工时与项目支出
  → 分析进度、成本和风险
  → 比较技术方案并由负责人确认
```

## 基础版功能

- 用户登录、用户信息、团队成员和四类角色权限
- 项目、项目成员、预算、里程碑和项目归档
- 任务看板、任务详情、评论、评审、测试和操作记录
- 任务前置关系、循环依赖检测、启动阻塞和延期影响分析
- 工作日志、岗位时薪、成员负载、分类支出和项目成本
- 任务逾期、依赖阻塞、成员过载、预算超支和 EVM 风险预警
- TCO、ROI、指标标准化、加权评分、推荐说明和人工确认
- 项目完成率、各状态任务数量、预算使用率、SPI、CPI、成本趋势与风险汇总

## 技术架构

```text
Vue 3 + TypeScript + Element Plus + ECharts
                         │ REST / JSON
Rust + Axum + Tokio + Serde + SQLx
                         │
                       SQLite
```

前端只负责录入和展示。权限判断、任务状态机、依赖图算法、成本与 EVM、风险判断、TCO/ROI、标准化、加权排序及推荐理由全部由 Rust 后端完成。金额写入和数据库结算使用整数分，概率与权重使用整数基点，避免用浮点数结算金额。

权限分为两层：系统角色控制用户管理、团队/项目创建等全局能力，`project_members.project_role` 控制项目内管理、开发和评审权限。非管理员只能访问自己加入的项目；项目归档后历史数据仍可查询，但项目写操作全部只读。

## 快速启动

环境要求：Rust 1.88+、Node.js 20.19.x（或 22.12+）、npm 10+。

```powershell
cd D:\RustFlow
./start.ps1
```

首次运行时，脚本会在缺少前端依赖时执行 `npm install`，并分别启动后端和前端。默认访问地址：

- 前端：<http://127.0.0.1:5173>
- 后端健康检查：<http://127.0.0.1:3100/health>

演示账号由后端首次初始化生成，密码统一为 `RustFlow123!`：`admin`、`manager`、`dev`、`reviewer`。

如需停止由脚本启动的服务：

```powershell
./stop.ps1
```

脚本输出与进程号保存在 `.run/`。项目启动脚本统一使用 `3100` 端口运行后端，前端 `/api` 也代理到 `http://127.0.0.1:3100`。如需手动启动，可分别执行：

```powershell
cd D:\RustFlow\backend
$env:BIND_ADDR = '127.0.0.1:3100'
cargo run

cd ..\frontend
npm install
npm run dev
```

后端默认数据库文件为 `backend/rustflow.db`。

## 前端构建与静态展示

进入 `frontend` 目录后，可以按使用场景选择构建方式：

```powershell
# 常规生产构建，输出到 frontend/dist
npm run build

# 单文件交互版，输出到 frontend/dist-single/RustFlow.html
npm run build:single

# 全页面静态展示版，输出到 frontend/dist-showcase
# 生成时需要先启动 3100 端口的后端，并确保本机安装 Microsoft Edge
npm run build:showcase
```

单文件交互版包含完整前端程序，直接打开时默认连接 `http://127.0.0.1:3100/api`；全页面静态展示版则将主要页面依次展开并内嵌到一个长 HTML 中，生成完成后可以脱离后端直接浏览。

提交前可分别执行：

```powershell
cd backend
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test

cd ../frontend
npm run build
```

## 文档导航

- [需求范围与验收标准](docs/requirements-and-acceptance.md)
- [系统架构、数据模型与接口约定](docs/architecture-and-data-model.md)
- [前端功能与交互说明](docs/frontend-interaction.md)
- [课程要求对应、团队分工与过程证据](docs/course-mapping-and-teamwork.md)
- [项目演示流程](docs/demo-script.md)

## 课程要求概览

| 课程要求 | 项目中的直接体现 | 报告证据 |
| --- | --- | --- |
| 计算机系统应用环境与开发工具 | Rust/Vue 前后端、数据库、认证、接口、测试和部署 | 环境表、构建测试截图、API 测试、Git 记录 |
| 多学科背景下的团队合作 | 项目经理、开发、评审/测试等角色进行任务交接和共同决策 | 分工表、Issue、提交、评审、会议纪要 |
| 工程管理原理与经济决策 | 里程碑、依赖、负载、风险、EVM、TCO、ROI、加权决策 | WBS、计算案例、预警截图和决策快照 |

## 交付边界

验收目标是“流程完整且计算可信”，不是堆叠人事或社交功能。基础版完成后，必须能够用一个项目案例演示从建项、分工到成本分析、风险传播和方案确认的全过程。人员推荐、设备借还、决策敏感性分析、报告导出等“争取完成”项均未纳入第一版，也不作为当前系统已实现功能描述。
