PRAGMA foreign_keys = ON;

CREATE TABLE users (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  username TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  display_name TEXT NOT NULL,
  role TEXT NOT NULL CHECK(role IN ('admin','manager','developer','reviewer')),
  hourly_rate_cents INTEGER NOT NULL DEFAULT 0 CHECK(hourly_rate_cents >= 0),
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE teams (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  owner_id INTEGER NOT NULL REFERENCES users(id),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE team_members (
  team_id INTEGER NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK(role IN ('manager','developer','reviewer')),
  weekly_capacity_hours REAL NOT NULL DEFAULT 40 CHECK(weekly_capacity_hours > 0),
  PRIMARY KEY(team_id,user_id)
);

CREATE TABLE projects (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  team_id INTEGER NOT NULL REFERENCES teams(id),
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  manager_id INTEGER NOT NULL REFERENCES users(id),
  start_date TEXT NOT NULL,
  end_date TEXT NOT NULL,
  budget_cents INTEGER NOT NULL CHECK(budget_cents >= 0),
  status TEXT NOT NULL DEFAULT 'planning' CHECK(status IN ('planning','active','completed','archived')),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE project_members (
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  project_role TEXT NOT NULL CHECK(project_role IN ('manager','developer','reviewer')),
  PRIMARY KEY(project_id,user_id)
);

CREATE TABLE milestones (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  start_date TEXT NOT NULL,
  end_date TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','active','completed'))
);

CREATE TABLE tasks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  milestone_id INTEGER REFERENCES milestones(id) ON DELETE SET NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  acceptance_criteria TEXT NOT NULL DEFAULT '',
  assignee_id INTEGER REFERENCES users(id),
  priority TEXT NOT NULL DEFAULT 'medium' CHECK(priority IN ('low','medium','high','critical')),
  status TEXT NOT NULL DEFAULT 'todo' CHECK(status IN ('todo','in_progress','review','testing','done')),
  planned_start TEXT NOT NULL,
  planned_end TEXT NOT NULL,
  estimated_hours REAL NOT NULL DEFAULT 0 CHECK(estimated_hours >= 0),
  progress INTEGER NOT NULL DEFAULT 0 CHECK(progress BETWEEN 0 AND 100),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE task_participants (
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  PRIMARY KEY(task_id,user_id)
);
CREATE TABLE task_dependencies (
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  depends_on_task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  PRIMARY KEY(task_id,depends_on_task_id),
  CHECK(task_id <> depends_on_task_id)
);
CREATE TABLE task_comments (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  author_id INTEGER NOT NULL REFERENCES users(id),
  content TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE task_reviews (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  reviewer_id INTEGER NOT NULL REFERENCES users(id),
  stage TEXT NOT NULL CHECK(stage IN ('review','testing')),
  result TEXT NOT NULL CHECK(result IN ('approved','rejected')),
  comment TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE worklogs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  user_id INTEGER NOT NULL REFERENCES users(id),
  work_date TEXT NOT NULL,
  hours REAL NOT NULL CHECK(hours > 0 AND hours <= 24),
  content TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE expenses (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  category TEXT NOT NULL CHECK(category IN ('device','cloud','purchase','other')),
  amount_cents INTEGER NOT NULL CHECK(amount_cents >= 0),
  occurred_on TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  created_by INTEGER NOT NULL REFERENCES users(id),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE risk_alerts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  task_id INTEGER REFERENCES tasks(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  level TEXT NOT NULL CHECK(level IN ('low','medium','high')),
  message TEXT NOT NULL,
  resolved INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE decisions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  analysis_years INTEGER NOT NULL DEFAULT 1 CHECK(analysis_years > 0),
  status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','evaluated','confirmed')),
  confirmed_option_id INTEGER,
  confirmation_reason TEXT,
  confirmed_by INTEGER REFERENCES users(id),
  confirmed_at TEXT,
  created_by INTEGER NOT NULL REFERENCES users(id),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE decision_options (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  decision_id INTEGER NOT NULL REFERENCES decisions(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  initial_cost_cents INTEGER NOT NULL DEFAULT 0,
  development_cost_cents INTEGER NOT NULL DEFAULT 0,
  annual_operation_cost_cents INTEGER NOT NULL DEFAULT 0,
  risk_probability_bps INTEGER NOT NULL DEFAULT 0 CHECK(risk_probability_bps BETWEEN 0 AND 10000),
  risk_loss_cents INTEGER NOT NULL DEFAULT 0,
  expected_benefit_cents INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE decision_metrics (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  decision_id INTEGER NOT NULL REFERENCES decisions(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  weight_bps INTEGER NOT NULL CHECK(weight_bps BETWEEN 1 AND 10000),
  direction TEXT NOT NULL CHECK(direction IN ('higher','lower')),
  unit TEXT NOT NULL DEFAULT ''
);
CREATE TABLE decision_values (
  option_id INTEGER NOT NULL REFERENCES decision_options(id) ON DELETE CASCADE,
  metric_id INTEGER NOT NULL REFERENCES decision_metrics(id) ON DELETE CASCADE,
  raw_value REAL NOT NULL,
  PRIMARY KEY(option_id,metric_id)
);
CREATE TABLE decision_results (
  decision_id INTEGER NOT NULL REFERENCES decisions(id) ON DELETE CASCADE,
  option_id INTEGER NOT NULL REFERENCES decision_options(id) ON DELETE CASCADE,
  tco_cents INTEGER NOT NULL,
  roi_bps INTEGER NOT NULL,
  total_score REAL NOT NULL,
  rank INTEGER NOT NULL,
  scores_json TEXT NOT NULL,
  advantages_json TEXT NOT NULL,
  disadvantages_json TEXT NOT NULL,
  evaluated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY(decision_id,option_id)
);

CREATE TABLE activity_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  actor_id INTEGER REFERENCES users(id),
  entity_type TEXT NOT NULL,
  entity_id INTEGER,
  action TEXT NOT NULL,
  detail TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_tasks_project ON tasks(project_id);
CREATE INDEX idx_worklogs_task ON worklogs(task_id);
CREATE INDEX idx_expenses_project ON expenses(project_id);
CREATE INDEX idx_activity_entity ON activity_logs(entity_type,entity_id);
