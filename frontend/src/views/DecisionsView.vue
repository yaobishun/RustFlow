<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import VChart from "vue-echarts";
import { use } from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";
import { BarChart, RadarChart } from "echarts/charts";
import {
  GridComponent,
  LegendComponent,
  TooltipComponent,
  RadarComponent,
} from "echarts/components";
import PageHeader from "../components/PageHeader.vue";
import PageState from "../components/PageState.vue";
import { baseChart, axisLabel } from "../charts/theme";
import { decisionApi } from "../api";
import { errorMessage } from "../api/http";
import { useAuthStore } from "../stores/auth";
import { useProjectStore } from "../stores/projects";
import type { Decision, DecisionMetric, DecisionOption } from "../types";
use([
  CanvasRenderer,
  BarChart,
  RadarChart,
  GridComponent,
  LegendComponent,
  TooltipComponent,
  RadarComponent,
]);
const auth = useAuthStore();
const projects = useProjectStore();
const canManage = computed(() =>
  ["admin", "manager"].includes(auth.user?.role || ""),
);
const activeProjects = computed(() =>
  projects.projects.filter((p) => p.status !== "archived"),
);
const selectedArchived = computed(
  () =>
    projects.projects.find((p) => p.id === selected.value?.project_id)
      ?.status === "archived",
);
const items = ref<Decision[]>([]);
const selected = ref<Decision>();
const loading = ref(true);
const error = ref("");
const dialog = ref(false);
const step = ref(0);
const saving = ref(false);
const form = reactive<{
  project_id?: number;
  title: string;
  description: string;
  analysis_years: number;
  metrics: DecisionMetric[];
  options: DecisionOption[];
}>({
  project_id: undefined,
  title: "",
  description: "",
  analysis_years: 3,
  metrics: [
    {
      name: "技术可行性",
      key: "feasibility",
      weight: 20,
      direction: "higher",
      unit: "分",
    },
    { name: "TCO", key: "tco", weight: 30, direction: "lower", unit: "元" },
    {
      name: "开发周期",
      key: "duration",
      weight: 15,
      direction: "lower",
      unit: "天",
    },
    {
      name: "安全性",
      key: "security",
      weight: 15,
      direction: "higher",
      unit: "分",
    },
    {
      name: "扩展能力",
      key: "scalability",
      weight: 10,
      direction: "higher",
      unit: "分",
    },
    { name: "ROI", key: "roi", weight: 10, direction: "higher", unit: "%" },
  ],
  options: [],
});
const totalWeight = computed(() =>
  form.metrics.reduce((s, x) => s + Number(x.weight || 0), 0),
);
async function load() {
  loading.value = true;
  error.value = "";
  try {
    const list = await decisionApi.list();
    items.value = await Promise.all(list.map((x) => decisionApi.get(x.id)));
    const current = selected.value?.id;
    selected.value =
      items.value.find((x) => x.id === current) || items.value[0];
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loading.value = false;
  }
}
function open() {
  Object.assign(form, {
    project_id: projects.selectedId,
    title: "",
    description: "",
    analysis_years: 3,
    options: [],
  });
  step.value = 0;
  dialog.value = true;
}
function addOption() {
  form.options.push({
    name: `方案 ${form.options.length + 1}`,
    initial_cost: 0,
    development_cost: 0,
    operation_cost: 0,
    risk_probability: 0,
    risk_loss: 0,
    expected_benefit: 0,
    values: { feasibility: 0, duration: 0, security: 0, scalability: 0 },
  });
}
async function create() {
  if (totalWeight.value !== 100)
    return ElMessage.warning("评价指标权重合计必须等于 100%");
  saving.value = true;
  try {
    const created = await decisionApi.create(form);
    dialog.value = false;
    selected.value = created;
    ElMessage.success("决策问题已创建");
    await load();
  } catch (e) {
    ElMessage.error(errorMessage(e));
  } finally {
    saving.value = false;
  }
}
async function evaluate() {
  if (!selected.value) return;
  try {
    selected.value = await decisionApi.evaluate(selected.value.id);
    ElMessage.success("Rust 评价计算已完成");
    await load();
  } catch (e) {
    ElMessage.error(errorMessage(e));
  }
}
async function confirm(option: DecisionOption) {
  if (!selected.value || !option.id) return;
  try {
    const { value } = await ElMessageBox.prompt(
      "请填写最终选择理由（可与系统推荐不同）",
      "确认最终方案",
      {
        inputType: "textarea",
        inputValidator: (v) => Boolean(v) || "请填写选择理由",
      },
    );
    selected.value = await decisionApi.confirm(
      selected.value.id,
      option.id,
      value,
    );
    ElMessage.success("最终决策已保存");
    await load();
  } catch (e) {
    if (e !== "cancel" && e !== "close") ElMessage.error(errorMessage(e));
  }
}
const scoreChart = computed(() => ({
  ...baseChart,
  tooltip: { ...baseChart.tooltip, trigger: "axis" },
  grid: { left: 35, right: 15, top: 24, bottom: 30 },
  xAxis: {
    type: "category",
    data: selected.value?.options.map((x) => x.name) || [],
    axisLine: { lineStyle: { color: "#e7eaf1" } },
    axisTick: { show: false },
    axisLabel,
  },
  yAxis: {
    type: "value",
    max: 100,
    axisLabel,
    splitLine: { lineStyle: { color: "#e9ecf1", type: "dashed" } },
  },
  series: [
    {
      type: "bar",
      barWidth: 34,
      itemStyle: { color: "#2161ee", borderRadius: [6, 6, 0, 0] },
      data: selected.value?.options.map((x) => x.total_score || 0) || [],
    },
  ],
}));
const radarChart = computed(() => ({
  ...baseChart,
  tooltip: { ...baseChart.tooltip },
  legend: { ...baseChart.legend, bottom: 0 },
  radar: {
    indicator: (selected.value?.metrics || []).map((x) => ({
      name: x.name,
      max: 100,
    })),
    radius: "62%",
    axisName: { color: "#667085", fontSize: 12 },
    splitLine: { lineStyle: { color: "#e7eaf1" } },
    splitArea: { areaStyle: { color: ["#fafbfd", "#f3f5f8"] } },
  },
  series: [
    {
      type: "radar",
      data: (selected.value?.options || [])
        .filter((x) => x.scores)
        .map((x) => ({
          name: x.name,
          value: (selected.value?.metrics || []).map((m) =>
            Number(x.scores?.[m.key] ?? x.scores?.[m.name] ?? 0),
          ),
        })),
    },
  ],
}));
const money = (v?: number) => `¥${Number(v || 0).toLocaleString()}`;
onMounted(async () => {
  await projects.load().catch(() => undefined);
  await load();
});
</script>
<template>
  <div>
    <PageHeader
      title="技术方案决策中心"
      description="以 TCO、ROI、风险损失与多指标评分辅助工程决策"
      ><el-button
        v-if="canManage && activeProjects.length"
        type="primary"
        @click="open"
        >创建决策问题</el-button
      ></PageHeader
    >
    <PageState :loading :error @retry="load"
      ><div v-if="items.length" class="decision-layout">
        <aside class="decision-list">
          <button
            v-for="d in items"
            :key="d.id"
            :class="{ active: selected?.id === d.id }"
            @click="selected = d"
          >
            <div>
              <el-tag
                size="small"
                :type="
                  d.status === 'confirmed'
                    ? 'success'
                    : d.status === 'evaluated'
                      ? 'primary'
                      : 'info'
                "
                >{{
                  d.status === "confirmed"
                    ? "已确认"
                    : d.status === "evaluated"
                      ? "已评价"
                      : "草稿"
                }}</el-tag
              ><time>{{ d.analysis_years }} 年期</time>
            </div>
            <b>{{ d.title }}</b
            ><span>{{ d.options?.length || 0 }} 个候选方案</span>
          </button>
        </aside>
        <main v-if="selected" class="decision-main">
          <div class="decision-title">
            <div>
              <span class="eyebrow">DECISION CASE #{{ selected.id }}</span>
              <h2>{{ selected.title }}</h2>
              <p>{{ selected.description || "暂无决策说明" }}</p>
            </div>
            <el-button
              v-if="
                canManage &&
                !selectedArchived &&
                selected.status !== 'confirmed'
              "
              type="primary"
              @click="evaluate"
              >执行评价计算</el-button
            >
          </div>
          <el-alert
            v-if="selected.status === 'confirmed'"
            type="success"
            :closable="false"
            show-icon
            ><template #title
              >最终方案：{{
                selected.confirmed_option_name ||
                selected.options.find(
                  (x) => x.id === selected?.confirmed_option_id,
                )?.name
              }}</template
            >
            <p>
              确认理由：{{ selected.confirmation_reason || "未填写" }}
            </p></el-alert
          >
          <el-alert
            v-else-if="selected.recommendation"
            type="info"
            :closable="false"
            show-icon
            ><template #title>系统推荐：{{ selected.recommendation }}</template>
            <p>{{ selected.recommendation_reason }}</p></el-alert
          >
          <div
            v-if="selected.status !== 'draft'"
            class="grid two dashboard-charts"
          >
            <div class="panel">
              <h3>综合得分</h3>
              <VChart class="chart" :option="scoreChart" autoresize />
            </div>
            <div class="panel">
              <h3>指标标准分对比</h3>
              <VChart
                v-if="selected.options.some((x) => x.scores)"
                class="chart"
                :option="radarChart"
                autoresize
              /><el-empty v-else description="后端暂未返回标准分明细" />
            </div>
          </div>
          <div class="panel table-panel">
            <el-table :data="selected.options" empty-text="尚未添加候选方案"
              ><el-table-column label="排名" width="65"
                ><template #default="{ row, $index }">{{
                  row.rank || $index + 1
                }}</template></el-table-column
              ><el-table-column
                prop="name"
                label="候选方案"
                min-width="130"
              /><el-table-column label="TCO"
                ><template #default="{ row }">{{
                  row.tco === undefined ? "待计算" : money(row.tco)
                }}</template></el-table-column
              ><el-table-column label="ROI"
                ><template #default="{ row }">{{
                  row.roi === undefined
                    ? "待计算"
                    : `${Number(row.roi).toFixed(2)}%`
                }}</template></el-table-column
              ><el-table-column label="得分"
                ><template #default="{ row }"
                  ><b>{{
                    row.total_score === undefined
                      ? "—"
                      : Number(row.total_score).toFixed(1)
                  }}</b></template
                ></el-table-column
              ><el-table-column label="优势" min-width="190"
                ><template #default="{ row }"
                  ><span v-if="row.advantages?.length">{{
                    row.advantages.join("；")
                  }}</span
                  ><span v-else>—</span></template
                ></el-table-column
              ><el-table-column label="不足" min-width="150"
                ><template #default="{ row }"
                  ><span v-if="row.disadvantages?.length">{{
                    row.disadvantages.join("；")
                  }}</span
                  ><span v-else>—</span></template
                ></el-table-column
              ><el-table-column label="操作" width="105"
                ><template #default="{ row }"
                  ><el-button
                    v-if="
                      canManage &&
                      !selectedArchived &&
                      selected?.status === 'evaluated'
                    "
                    link
                    type="primary"
                    @click="confirm(row)"
                    >确认选择</el-button
                  ><el-tag
                    v-else-if="selected?.confirmed_option_id === row.id"
                    type="success"
                    >最终方案</el-tag
                  ></template
                ></el-table-column
              ></el-table
            >
          </div>
        </main>
      </div>
      <el-empty v-else description="暂无决策问题"
    /></PageState>
    <el-dialog
      v-model="dialog"
      title="创建技术方案决策"
      width="min(900px,96vw)"
      top="5vh"
      ><el-steps :active="step" align-center
        ><el-step title="决策信息" /><el-step title="评价指标" /><el-step
          title="候选方案"
      /></el-steps>
      <div v-if="step === 0" class="step-body">
        <el-form label-position="top"
          ><div class="form-grid">
            <el-form-item label="所属项目"
              ><el-select v-model="form.project_id"
                ><el-option
                  v-for="p in activeProjects"
                  :key="p.id"
                  :label="p.name"
                  :value="p.id" /></el-select></el-form-item
            ><el-form-item label="分析周期（年）"
              ><el-input-number
                v-model="form.analysis_years"
                :min="1"
                :max="10"
            /></el-form-item>
          </div>
          <el-form-item label="决策问题"
            ><el-input v-model="form.title" /></el-form-item
          ><el-form-item label="决策说明"
            ><el-input
              v-model="form.description"
              type="textarea" /></el-form-item
        ></el-form>
      </div>
      <div v-else-if="step === 1" class="step-body">
        <div class="weight-total" :class="{ invalid: totalWeight !== 100 }">
          当前权重合计 <b>{{ totalWeight }}%</b>
        </div>
        <el-table :data="form.metrics"
          ><el-table-column prop="name" label="指标" /><el-table-column
            prop="unit"
            label="单位" /><el-table-column label="方向"
            ><template #default="{ row }">{{
              row.direction === "higher" ? "越大越好" : "越小越好"
            }}</template></el-table-column
          ><el-table-column label="权重"
            ><template #default="{ row }"
              ><el-input-number
                v-model="row.weight"
                :min="0"
                :max="100" /></template></el-table-column
        ></el-table>
      </div>
      <div v-else class="step-body">
        <div class="option-tabs">
          <el-button @click="addOption">+ 添加候选方案</el-button>
        </div>
        <el-empty
          v-if="!form.options.length"
          description="至少添加两个候选方案"
        /><el-collapse v-else
          ><el-collapse-item
            v-for="(o, i) in form.options"
            :key="i"
            :title="o.name"
            :name="i"
            ><div class="form-grid triple">
              <el-form-item label="方案名称"
                ><el-input v-model="o.name" /></el-form-item
              ><el-form-item label="初始投入（元）"
                ><el-input-number
                  v-model="o.initial_cost"
                  :min="0" /></el-form-item
              ><el-form-item label="开发成本（元）"
                ><el-input-number
                  v-model="o.development_cost"
                  :min="0" /></el-form-item
              ><el-form-item label="年运维成本（元）"
                ><el-input-number
                  v-model="o.operation_cost"
                  :min="0" /></el-form-item
              ><el-form-item label="风险概率（%）"
                ><el-input-number
                  v-model="o.risk_probability"
                  :min="0"
                  :max="100" /></el-form-item
              ><el-form-item label="风险损失（元）"
                ><el-input-number
                  v-model="o.risk_loss"
                  :min="0" /></el-form-item
              ><el-form-item label="预期收益（元）"
                ><el-input-number
                  v-model="o.expected_benefit"
                  :min="0" /></el-form-item
              ><el-form-item label="技术可行性"
                ><el-input-number
                  v-model="o.values.feasibility"
                  :min="0"
                  :max="100" /></el-form-item
              ><el-form-item label="开发周期（天）"
                ><el-input-number
                  v-model="o.values.duration"
                  :min="1" /></el-form-item
              ><el-form-item label="安全性"
                ><el-input-number
                  v-model="o.values.security"
                  :min="0"
                  :max="100" /></el-form-item
              ><el-form-item label="扩展能力"
                ><el-input-number
                  v-model="o.values.scalability"
                  :min="0"
                  :max="100"
              /></el-form-item></div></el-collapse-item
        ></el-collapse>
      </div>
      <template #footer
        ><el-button v-if="step > 0" @click="step--">上一步</el-button
        ><el-button
          v-if="step < 2"
          type="primary"
          :disabled="
            (step === 0 && !form.title) || (step === 1 && totalWeight !== 100)
          "
          @click="step++"
          >下一步</el-button
        ><el-button
          v-else
          type="primary"
          :loading="saving"
          :disabled="form.options.length < 2"
          @click="create"
          >创建决策</el-button
        ></template
      ></el-dialog
    >
  </div>
</template>
