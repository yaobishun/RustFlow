-- 方案决策硬约束：给指标加可选阈值，命中即标记方案不可行。
ALTER TABLE decision_metrics ADD COLUMN threshold REAL;

-- 评价结果记录可行性及违反的约束（指标名列表 JSON）。
ALTER TABLE decision_results ADD COLUMN feasible INTEGER NOT NULL DEFAULT 1;
ALTER TABLE decision_results ADD COLUMN violations_json TEXT NOT NULL DEFAULT '[]';
