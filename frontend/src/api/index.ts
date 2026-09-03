import { http, unwrap } from "./http";
import type {
  Comment,
  CostSummary,
  Dashboard,
  Decision,
  DecisionMetric,
  DecisionOption,
  Expense,
  MemberLoad,
  Milestone,
  Project,
  Risk,
  Task,
  Team,
  User,
  Worklog,
} from "../types";

const get = async <T>(url: string, params?: object) =>
  unwrap<T>(await http.get(url, { params }));
const post = async <T>(url: string, body?: object) =>
  unwrap<T>(await http.post(url, body));
const put = async <T>(url: string, body?: object) =>
  unwrap<T>(await http.put(url, body));

export const authApi = {
  login: (body: { username: string; password: string }) =>
    post<{ token: string; user: User }>("/auth/login", body),
  me: () => get<User>("/auth/me"),
  logout: () => post<{ ok: boolean }>("/auth/logout"),
};
export const peopleApi = {
  users: () => get<User[]>("/users"),
  createUser: (body: any) =>
    post("/users", {
      ...body,
      hourly_rate_cents: Math.round(Number(body.hourly_rate || 0) * 100),
      hourly_rate: undefined,
    }),
  updateUser: (id: number, body: any) =>
    put(`/users/${id}`, {
      ...body,
      hourly_rate_cents:
        body.hourly_rate === undefined
          ? undefined
          : Math.round(body.hourly_rate * 100),
      hourly_rate: undefined,
    }),
  teams: () => get<Team[]>("/teams"),
  createTeam: (body: object) => post("/teams", body),
  teamMembers: (id: number) => get<User[]>(`/teams/${id}/members`),
  addTeamMember: (id: number, body: object) =>
    post(`/teams/${id}/members`, body),
};
export const dashboardApi = {
  get: async (project_id?: number) => {
    const x = await get<any>(
      "/dashboard",
      project_id ? { project_id } : undefined,
    );
    const e = x.evm || {};
    return {
      project_count: 1,
      completion_rate: x.completion_rate,
      budget_usage_rate: x.budget_usage_rate,
      actual_cost: x.actual_cost,
      spi: e.spi ?? null,
      cpi: e.cpi ?? null,
      eac: e.eac ?? (e.eac_cents == null ? null : e.eac_cents / 100),
      overdue_tasks:
        x.overdue_task_count ??
        (x.latest_risks || []).filter((r: any) => r.kind === "overdue").length,
      high_risks: x.high_risk_count,
      task_distribution: x.task_counts || {},
      cost_trend: x.cost_trend || [],
      member_loads: x.member_loads || [],
      milestones: x.upcoming_milestones || [],
      latest_risks: (x.latest_risks || []).map(mapRisk),
    } as Dashboard;
  },
};
export const projectApi = {
  list: () => get<Project[]>("/projects"),
  get: (id: number) => get<Project>(`/projects/${id}`),
  create: (body: Partial<Project>) =>
    post<Project>("/projects", {
      ...body,
      budget_cents: Math.round(Number(body.budget || 0) * 100),
      budget: undefined,
    }),
  update: (id: number, body: Partial<Project>) =>
    put<Project>(`/projects/${id}`, {
      ...body,
      budget_cents:
        body.budget === undefined ? undefined : Math.round(body.budget * 100),
      budget: undefined,
    }),
  archive: (id: number) =>
    put<Project>(`/projects/${id}`, { status: "archived" }),
  milestones: (id: number) => get<Milestone[]>(`/projects/${id}/milestones`),
  createMilestone: (id: number, body: Partial<Milestone>) =>
    post<Milestone>(`/projects/${id}/milestones`, body),
  members: async (id: number) =>
    (await get<any[]>(`/projects/${id}/members`)).map(
      (x) =>
        ({
          id: x.id,
          name: x.display_name || x.name,
          role: x.project_role || x.role,
          hourly_rate: x.hourly_rate || 0,
          assigned_hours: x.assigned_hours || 0,
          available_hours: x.capacity || x.available_hours || 40,
          load_rate: x.load_rate || 0,
          active_tasks: x.active_tasks || 0,
        }) as MemberLoad,
    ),
  addMember: (id: number, body: object) =>
    post(`/projects/${id}/members`, body),
};
export const taskApi = {
  list: (project_id: number) => get<Task[]>(`/projects/${project_id}/tasks`),
  get: (id: number) => get<Task>(`/tasks/${id}`),
  create: (body: Partial<Task>) =>
    post<Task>(`/projects/${body.project_id}/tasks`, body),
  update: (id: number, body: Partial<Task>) => put<Task>(`/tasks/${id}`, body),
  transition: (id: number, status: string) =>
    post<Task>(`/tasks/${id}/transition`, { to_status: status }),
  comments: (id: number) => get<Comment[]>(`/tasks/${id}/comments`),
  addComment: (id: number, content: string) =>
    post<Comment>(`/tasks/${id}/comments`, { content }),
  review: (id: number, action: "approve" | "reject", comment: string) =>
    post<Task>(`/tasks/${id}/review`, {
      result: action === "approve" ? "approved" : "rejected",
      comment,
    }),
  dependencies: (id: number) => get<Task[]>(`/tasks/${id}/dependencies`),
  addDependency: (id: number, depends_on_task_id: number) =>
    post(`/tasks/${id}/dependencies`, { depends_on_task_id }),
};
export const financeApi = {
  summary: async (project_id: number) => {
    const raw = await get<any>(`/projects/${project_id}/finance/summary`);
    const e = raw.evm || {};
    return {
      budget: raw.budget,
      labor_cost: raw.labor_cost,
      equipment_cost: raw.equipment_cost ?? raw.device_cost ?? 0,
      cloud_cost: raw.cloud_cost ?? 0,
      procurement_cost: raw.procurement_cost ?? raw.purchase_cost ?? 0,
      other_cost: raw.other_cost ?? 0,
      actual_cost: raw.actual_cost,
      remaining_budget: raw.remaining_budget,
      pv: e.pv ?? (e.pv_cents == null ? 0 : e.pv_cents / 100),
      ev: e.ev ?? (e.ev_cents == null ? 0 : e.ev_cents / 100),
      ac: e.ac ?? (e.ac_cents == null ? raw.actual_cost : e.ac_cents / 100),
      spi: e.spi,
      cpi: e.cpi,
      eac: e.eac ?? (e.eac_cents == null ? 0 : e.eac_cents / 100),
    } as CostSummary;
  },
  worklogs: async (project_id: number) =>
    (await get<any[]>(`/projects/${project_id}/worklogs`)).map(
      (x) => ({ ...x, member_name: x.member_name || x.user_name }) as Worklog,
    ),
  addWorklog: (project_id: number, body: object) =>
    post<Worklog>(`/projects/${project_id}/worklogs`, body),
  expenses: (project_id: number) =>
    get<Expense[]>(`/projects/${project_id}/expenses`),
  addExpense: (project_id: number, body: any) =>
    post<Expense>(`/projects/${project_id}/expenses`, {
      ...body,
      amount_cents: Math.round(Number(body.amount || 0) * 100),
      amount: undefined,
      project_id: undefined,
    }),
};
const mapRisk = (x: any, i: number): Risk => ({
  id: x.id || i + 1,
  type: x.type || x.kind,
  level: x.level,
  title:
    x.title ||
    x.task_title ||
    (
      {
        overdue: "任务逾期",
        dependency_blocked: "依赖阻塞",
        overload: "成员过载",
        budget_overrun: "预算超支",
        forecast_overrun: "完工成本预警",
      } as Record<string, string>
    )[x.kind] ||
    "工程风险",
  reason: x.reason || x.message,
  related_task: x.related_task || x.task_title,
  created_at: x.created_at || "实时计算",
  resolved: Boolean(x.resolved),
  affected_tasks: x.affected_tasks,
});
export const riskApi = {
  list: async (project_id: number) =>
    (await get<any[]>(`/projects/${project_id}/risks`)).map(mapRisk),
  rescan: async (project_id: number) => {
    const raw = await post<any>("/risks/scan", { project_id });
    return (raw.risks || raw).map(mapRisk);
  },
};

type DecisionDraft = {
  project_id?: number;
  title: string;
  description: string;
  analysis_years: number;
  metrics: DecisionMetric[];
  options: DecisionOption[];
};
function decisionPayload(d: DecisionDraft) {
  return {
    project_id: d.project_id,
    title: d.title,
    description: d.description,
    analysis_years: d.analysis_years,
    metrics: d.metrics.map((m) => ({
      name: m.name,
      weight_bps: Math.round(m.weight * 100),
      direction: m.direction,
      unit: m.unit,
    })),
    options: d.options.map((o) => ({
      name: o.name,
      description: "",
      initial_cost_cents: Math.round(o.initial_cost * 100),
      development_cost_cents: Math.round(o.development_cost * 100),
      annual_operation_cost_cents: Math.round(o.operation_cost * 100),
      risk_probability_bps: Math.round(o.risk_probability * 100),
      risk_loss_cents: Math.round(o.risk_loss * 100),
      expected_benefit_cents: Math.round(o.expected_benefit * 100),
      values: d.metrics.map((m) =>
        m.key === "tco" || m.key === "roi" ? 0 : Number(o.values[m.key] || 0),
      ),
    })),
  };
}
async function fullDecision(id: number): Promise<Decision> {
  const d = await get<any>(`/decisions/${id}`);
  const byId = new Map((d.results || []).map((r: any) => [r.option_id, r]));
  const metricKeys = new Map(
    (d.metrics || []).map((m: any) => [m.id, m.key || m.name]),
  );
  d.options = (d.options || []).map((o: any) => ({
    ...o,
    initial_cost: o.initial_cost_cents / 100,
    development_cost: o.development_cost_cents / 100,
    operation_cost: o.annual_operation_cost_cents / 100,
    risk_probability: o.risk_probability_bps / 100,
    risk_loss: o.risk_loss_cents / 100,
    expected_benefit: o.expected_benefit_cents / 100,
    values: Array.isArray(o.values)
      ? Object.fromEntries(
          o.values.map((v: any) => [metricKeys.get(v.metric_id), v.raw_value]),
        )
      : o.values || {},
    ...(byId.get(o.id) || {}),
  }));
  const top = [...(d.results || [])].sort(
    (a: any, b: any) => a.rank - b.rank,
  )[0];
  if (top) {
    d.recommendation = top.option_name;
    d.recommendation_reason = `综合得分 ${Number(top.total_score).toFixed(1)}，TCO ¥${Number(top.tco).toLocaleString()}，ROI ${Number(top.roi).toFixed(2)}%。${top.advantages?.join("；") || ""}`;
  }
  return d as Decision;
}
export const decisionApi = {
  list: () => get<Decision[]>("/decisions"),
  get: fullDecision,
  create: async (body: DecisionDraft) => {
    const created = await post<{ id: number }>(
      "/decisions",
      decisionPayload(body),
    );
    return fullDecision(created.id);
  },
  evaluate: async (id: number) => {
    await post(`/decisions/${id}/evaluate`);
    return fullDecision(id);
  },
  confirm: async (id: number, option_id: number, reason: string) => {
    await post(`/decisions/${id}/confirm`, { option_id, reason });
    return fullDecision(id);
  },
};
