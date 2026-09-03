<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { ElMessage } from "element-plus";
import VChart from "vue-echarts";
import { use } from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";
import { PieChart } from "echarts/charts";
import { LegendComponent, TooltipComponent } from "echarts/components";
import PageHeader from "../components/PageHeader.vue";
import PageState from "../components/PageState.vue";
import MetricCard from "../components/MetricCard.vue";
import { financeApi, taskApi } from "../api";
import { errorMessage } from "../api/http";
import { useProjectStore } from "../stores/projects";
import { useAuthStore } from "../stores/auth";
import type { CostSummary, Expense, Task, Worklog } from "../types";

use([CanvasRenderer, PieChart, LegendComponent, TooltipComponent]);
const projects = useProjectStore();
const auth = useAuthStore();
const canExpense = computed(() =>
  ["admin", "manager"].includes(auth.user?.role || ""),
);
const canWorklog = computed(() =>
  ["admin", "manager", "developer"].includes(auth.user?.role || ""),
);
const archived = computed(
  () =>
    projects.projects.find((p) => p.id === projects.selectedId)?.status ===
    "archived",
);
const summary = ref<CostSummary>();
const worklogs = ref<Worklog[]>([]);
const expenses = ref<Expense[]>([]);
const tasks = ref<Task[]>([]);
const loading = ref(true);
const error = ref("");
const tab = ref("worklog");
const dialog = ref<"work" | "expense" | "">("");
const work = reactive({
  task_id: undefined as number | undefined,
  work_date: "",
  hours: 1,
  content: "",
});
const expense = reactive({
  category: "cloud",
  description: "",
  amount: 0,
  occurred_on: "",
});
async function projectId() {
  if (!projects.selectedId) await projects.load();
  if (!projects.selectedId) throw new Error("暂无可用项目，请先创建项目");
  return projects.selectedId;
}
async function load() {
  loading.value = true;
  error.value = "";
  try {
    const id = await projectId();
    const [s, w, e, t] = await Promise.all([
      financeApi.summary(id),
      financeApi.worklogs(id),
      financeApi.expenses(id),
      taskApi.list(id),
    ]);
    worklogs.value = w;
    expenses.value = e;
    tasks.value = t;
    s.equipment_cost = e
      .filter((x) => x.category === "device")
      .reduce((n, x) => n + x.amount, 0);
    s.cloud_cost = e
      .filter((x) => x.category === "cloud")
      .reduce((n, x) => n + x.amount, 0);
    s.procurement_cost = e
      .filter((x) => x.category === "purchase")
      .reduce((n, x) => n + x.amount, 0);
    s.other_cost = e
      .filter((x) => x.category === "other")
      .reduce((n, x) => n + x.amount, 0);
    summary.value = s;
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loading.value = false;
  }
}
async function addWork() {
  try {
    await financeApi.addWorklog(await projectId(), work);
    dialog.value = "";
    ElMessage.success("工时已记录");
    await load();
  } catch (e) {
    ElMessage.error(errorMessage(e));
  }
}
async function addExpense() {
  try {
    await financeApi.addExpense(await projectId(), expense);
    dialog.value = "";
    ElMessage.success("支出已记录");
    await load();
  } catch (e) {
    ElMessage.error(errorMessage(e));
  }
}
const chart = computed(() => ({
  tooltip: { trigger: "item", formatter: "{b}<br/>¥{c}（{d}%）" },
  legend: { bottom: 0 },
  series: [
    {
      type: "pie",
      radius: ["48%", "72%"],
      center: ["50%", "43%"],
      itemStyle: { borderRadius: 7, borderColor: "#fff", borderWidth: 3 },
      data: summary.value
        ? [
            { name: "人力", value: summary.value.labor_cost },
            { name: "设备", value: summary.value.equipment_cost },
            { name: "云服务", value: summary.value.cloud_cost },
            { name: "采购", value: summary.value.procurement_cost },
            { name: "其他", value: summary.value.other_cost },
          ]
        : [],
    },
  ],
}));
const money = (v: number) => `¥${Number(v || 0).toLocaleString()}`;
onMounted(() => {
  load();
  window.addEventListener("rustflow:project-change", load);
});
onBeforeUnmount(() =>
  window.removeEventListener("rustflow:project-change", load),
);
</script>
<template>
  <div>
    <PageHeader
      title="工时、预算与成本"
      description="用工时记录连接项目投入、预算偏差与挣值绩效"
      ><el-button v-if="canExpense && !archived" @click="dialog = 'expense'"
        >记录支出</el-button
      ><el-button
        v-if="canWorklog && !archived"
        type="primary"
        @click="dialog = 'work'"
        >填写工时</el-button
      ></PageHeader
    >
    <PageState :loading :error @retry="load"
      ><template v-if="summary">
        <div class="metric-grid">
          <MetricCard
            label="项目预算"
            :value="money(summary.budget)"
            :note="`剩余 ${money(summary.remaining_budget)}`"
          /><MetricCard
            label="实际成本 AC"
            :value="money(summary.actual_cost)"
            :note="`人力成本 ${money(summary.labor_cost)}`"
            tone="cyan"
          /><MetricCard
            label="进度绩效 SPI"
            :value="summary.spi?.toFixed(2) ?? '—'"
            :note="`PV ${money(summary.pv)} · EV ${money(summary.ev)}`"
            :tone="summary.spi !== null && summary.spi < 1 ? 'amber' : 'blue'"
          /><MetricCard
            label="预计完工 EAC"
            :value="summary.eac ? money(summary.eac) : '—'"
            :note="`成本绩效 CPI ${summary.cpi?.toFixed(2) ?? '—'}`"
            :tone="summary.eac > summary.budget ? 'red' : 'cyan'"
          />
        </div>
        <div class="grid costs-layout">
          <div class="panel">
            <div class="panel-head">
              <div>
                <h3>成本构成</h3>
                <p>实际投入按费用类型拆分</p>
              </div>
            </div>
            <VChart class="chart" :option="chart" autoresize />
          </div>
          <div class="panel evm-panel">
            <div class="panel-head">
              <div>
                <h3>EVM 健康诊断</h3>
                <p>进度与成本联动预警</p>
              </div>
            </div>
            <div class="evm-gauge">
              <div
                :class="
                  summary.spi !== null && summary.spi < 1 ? 'bad' : 'good'
                "
              >
                <span>SPI</span><b>{{ summary.spi?.toFixed(2) ?? "—" }}</b
                ><small>{{
                  summary.spi !== null && summary.spi < 1
                    ? "进度落后"
                    : "进度正常"
                }}</small>
              </div>
              <div
                :class="
                  summary.cpi !== null && summary.cpi < 1 ? 'bad' : 'good'
                "
              >
                <span>CPI</span><b>{{ summary.cpi?.toFixed(2) ?? "—" }}</b
                ><small>{{
                  summary.cpi !== null && summary.cpi < 1
                    ? "成本效率偏低"
                    : "成本效率正常"
                }}</small>
              </div>
            </div>
            <div class="formula-note">
              SPI = EV ÷ PV　　CPI = EV ÷ AC　　EAC = BAC ÷ CPI
            </div>
          </div>
        </div>
        <div class="panel table-panel">
          <el-tabs v-model="tab"
            ><el-tab-pane label="工时记录" name="worklog"
              ><el-table :data="worklogs" empty-text="暂无工时记录"
                ><el-table-column
                  prop="work_date"
                  label="日期"
                /><el-table-column
                  prop="member_name"
                  label="成员"
                /><el-table-column
                  prop="task_title"
                  label="关联任务"
                  min-width="180"
                /><el-table-column
                  prop="content"
                  label="工作内容"
                  min-width="220"
                /><el-table-column prop="hours" label="工时"
                  ><template #default="{ row }"
                    >{{ row.hours }}h</template
                  ></el-table-column
                ></el-table
              ></el-tab-pane
            ><el-tab-pane label="支出明细" name="expense"
              ><el-table :data="expenses" empty-text="暂无支出记录"
                ><el-table-column
                  prop="occurred_on"
                  label="日期"
                /><el-table-column
                  prop="category"
                  label="类型"
                /><el-table-column
                  prop="description"
                  label="说明"
                  min-width="260"
                /><el-table-column label="金额"
                  ><template #default="{ row }">{{
                    money(row.amount)
                  }}</template></el-table-column
                ></el-table
              ></el-tab-pane
            ></el-tabs
          >
        </div>
      </template></PageState
    >
    <el-dialog
      v-model="dialog"
      :title="dialog === 'work' ? '填写工时' : '记录项目支出'"
      width="min(520px,92vw)"
      ><el-form v-if="dialog === 'work'" label-position="top"
        ><el-form-item label="关联任务"
          ><el-select v-model="work.task_id" filterable
            ><el-option
              v-for="t in tasks"
              :key="t.id"
              :label="t.title"
              :value="t.id" /></el-select></el-form-item
        ><el-form-item label="工作日期"
          ><el-date-picker
            v-model="work.work_date"
            value-format="YYYY-MM-DD" /></el-form-item
        ><el-form-item label="工时"
          ><el-input-number
            v-model="work.hours"
            :min="0.5"
            :step="0.5" /></el-form-item
        ><el-form-item label="工作内容"
          ><el-input
            v-model="work.content"
            type="textarea" /></el-form-item></el-form
      ><el-form v-else label-position="top"
        ><el-form-item label="支出类型"
          ><el-select v-model="expense.category"
            ><el-option label="设备" value="device" /><el-option
              label="云服务"
              value="cloud" /><el-option
              label="采购"
              value="purchase" /><el-option
              label="其他"
              value="other" /></el-select></el-form-item
        ><el-form-item label="金额（元）"
          ><el-input-number
            v-model="expense.amount"
            :min="0"
            :step="100" /></el-form-item
        ><el-form-item label="发生日期"
          ><el-date-picker
            v-model="expense.occurred_on"
            value-format="YYYY-MM-DD" /></el-form-item
        ><el-form-item label="支出说明"
          ><el-input v-model="expense.description" /></el-form-item></el-form
      ><template #footer
        ><el-button @click="dialog = ''">取消</el-button
        ><el-button
          type="primary"
          @click="dialog === 'work' ? addWork() : addExpense()"
          >保存</el-button
        ></template
      ></el-dialog
    >
  </div>
</template>
