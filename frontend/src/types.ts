export type Role = "admin" | "manager" | "developer" | "reviewer";
export type TaskStatus = "todo" | "in_progress" | "review" | "testing" | "done";

export interface User {
  id: number;
  username: string;
  name?: string;
  display_name?: string;
  role: Role;
  email?: string;
  avatar?: string;
  hourly_rate?: number;
  active?: boolean;
}
export interface Team {
  id: number;
  name: string;
  description?: string;
  owner_id: number;
  owner_name: string;
  member_count: number;
}
export interface Project {
  id: number;
  team_id?: number;
  manager_id?: number;
  name: string;
  description?: string;
  status: string;
  manager_name?: string;
  start_date: string;
  end_date: string;
  budget: number;
  budget_cents?: number;
  completion_rate: number;
}
export interface Milestone {
  id: number;
  name: string;
  due_date: string;
  completion_rate: number;
  status: string;
}
export interface Task {
  id: number;
  project_id: number;
  milestone_id?: number;
  title: string;
  description?: string;
  acceptance_criteria?: string;
  assignee_id?: number;
  assignee_name?: string;
  participants?: User[];
  priority: "low" | "medium" | "high" | "critical";
  status: TaskStatus;
  progress: number;
  estimated_hours: number;
  actual_hours?: number;
  planned_start: string;
  planned_end: string;
  blocked?: boolean;
  affected_count?: number;
}
export interface Comment {
  id: number;
  author_name: string;
  content: string;
  created_at: string;
  kind?: "comment" | "review" | "system";
}
export interface MemberLoad {
  id: number;
  name: string;
  role: Role;
  hourly_rate: number;
  assigned_hours: number;
  available_hours: number;
  load_rate: number;
  active_tasks: number;
}
export interface Worklog {
  id: number;
  task_id: number;
  task_title: string;
  member_name: string;
  work_date: string;
  hours: number;
  content: string;
}
export interface Expense {
  id: number;
  category: string;
  description: string;
  amount: number;
  occurred_on: string;
}
export interface Risk {
  id: number;
  type: string;
  level: "low" | "medium" | "high";
  title: string;
  reason: string;
  related_task?: string;
  created_at: string;
  resolved: boolean;
  affected_tasks?: Array<{ id: number; title: string; assignee_name?: string }>;
}
export interface Dashboard {
  project_count: number;
  completion_rate: number;
  budget_usage_rate: number;
  actual_cost: number;
  spi: number | null;
  cpi: number | null;
  eac: number | null;
  overdue_tasks: number;
  high_risks: number;
  task_distribution: Record<string, number>;
  cost_trend: Array<{ date: string; value: number }>;
  member_loads: MemberLoad[];
  milestones: Milestone[];
  latest_risks: Risk[];
}
export interface CostSummary {
  budget: number;
  labor_cost: number;
  equipment_cost: number;
  cloud_cost: number;
  procurement_cost: number;
  other_cost: number;
  actual_cost: number;
  remaining_budget: number;
  pv: number;
  ev: number;
  ac: number;
  spi: number | null;
  cpi: number | null;
  eac: number;
}
export interface DecisionOption {
  id?: number;
  name: string;
  initial_cost: number;
  development_cost: number;
  operation_cost: number;
  risk_probability: number;
  risk_loss: number;
  expected_benefit: number;
  values: Record<string, number>;
  scores?: Record<string, number>;
  raw_values?: Record<string, number>;
  tco?: number;
  roi?: number;
  total_score?: number;
  rank?: number;
  feasible?: boolean;
  violations?: string[];
  advantages?: string[];
  disadvantages?: string[];
}
export interface DecisionMetric {
  id?: number;
  name: string;
  key: string;
  weight: number;
  direction: "higher" | "lower";
  unit: string;
  threshold?: number;
}
export interface Decision {
  id: number;
  project_id: number;
  title: string;
  description?: string;
  analysis_years: number;
  status: "draft" | "evaluated" | "confirmed";
  options: DecisionOption[];
  metrics: DecisionMetric[];
  recommendation?: string;
  recommendation_reason?: string;
  confirmed_option_id?: number;
  confirmed_option_name?: string;
  confirmation_reason?: string;
}
