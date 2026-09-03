<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import VChart from "vue-echarts";
import { use } from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";
import { BarChart, LineChart, PieChart } from "echarts/charts";
import {
  GridComponent,
  LegendComponent,
  TooltipComponent,
} from "echarts/components";
import PageHeader from "../components/PageHeader.vue";
import PageState from "../components/PageState.vue";
import MetricCard from "../components/MetricCard.vue";
import { dashboardApi } from "../api";
import { errorMessage } from "../api/http";
import type { Dashboard } from "../types";
import { useProjectStore } from "../stores/projects";
use([
  CanvasRenderer,
  BarChart,
  LineChart,
  PieChart,
  GridComponent,
  LegendComponent,
  TooltipComponent,
]);
const projects = useProjectStore();
const data = ref<Dashboard>();
const loading = ref(true);
const error = ref("");
async function load() {
  loading.value = true;
  error.value = "";
  try {
    if (!projects.selectedId) await projects.load();
    data.value = await dashboardApi.get(projects.selectedId);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loading.value = false;
  }
}
onMounted(() => {
  load();
  window.addEventListener("rustflow:project-change", load);
});
const taskChart = computed(() => ({
  tooltip: { trigger: "item" },
  legend: { bottom: 0 },
  series: [
    {
      type: "pie",
      radius: ["52%", "74%"],
      center: ["50%", "43%"],
      itemStyle: { borderRadius: 7, borderWidth: 3, borderColor: "#fff" },
      data: Object.entries(data.value?.task_distribution || {}).map(
        ([name, value]) => ({ name, value }),
      ),
    },
  ],
}));
const costChart = computed(() => ({
  tooltip: { trigger: "axis" },
  grid: { left: 30, right: 15, top: 20, bottom: 28 },
  xAxis: {
    type: "category",
    data: (data.value?.cost_trend || []).map((x) => x.date),
    axisLine: { show: false },
    axisTick: { show: false },
  },
  yAxis: {
    type: "value",
    axisLine: { show: false },
    splitLine: { lineStyle: { color: "#eef1f6" } },
  },
  series: [
    {
      type: "line",
      smooth: true,
      symbolSize: 7,
      areaStyle: { color: "rgba(33,97,238,.1)" },
      lineStyle: { width: 3, color: "#2161ee" },
      data: (data.value?.cost_trend || []).map((x) => x.value),
    },
  ],
}));
const pct = (v: number) => `${Number(v || 0).toFixed(1)}%`;
const money = (v: number) => `¥${Number(v || 0).toLocaleString()}`;
</script>
<template>
  <div>
    <PageHeader
      title="综合仪表盘"
      description="集中掌握项目进度、成本效率与风险态势"
      ><el-button @click="load">刷新数据</el-button></PageHeader
    ><PageState :loading :error @retry="load">
      <div v-if="data" class="metric-grid">
        <MetricCard
          label="项目完成率"
          :value="pct(data.completion_rate)"
          note="当前所选项目"
        /><MetricCard
          label="预算使用率"
          :value="pct(data.budget_usage_rate)"
          :note="`已发生成本 ${money(data.actual_cost)}`"
          tone="cyan"
        /><MetricCard
          label="进度绩效 SPI"
          :value="data.spi?.toFixed(2) ?? '—'"
          :note="
            data.spi !== null && data.spi < 1
              ? '进度存在偏差'
              : '进度处于计划内'
          "
          :tone="data.spi !== null && data.spi < 1 ? 'amber' : 'blue'"
        /><MetricCard
          label="成本绩效 CPI"
          :value="data.cpi?.toFixed(2) ?? '—'"
          :note="data.eac ? `预计完工 ${money(data.eac)}` : '尚无有效数据'"
          :tone="data.cpi !== null && data.cpi < 1 ? 'red' : 'cyan'"
        />
      </div>
      <div v-if="data" class="grid two dashboard-charts">
        <div class="panel">
          <div class="panel-head">
            <div>
              <h3>任务状态分布</h3>
              <p>当前团队任务流转概况</p>
            </div>
            <span class="count-badge"
              >{{
                Object.values(data.task_distribution || {}).reduce(
                  (a, b) => a + b,
                  0,
                )
              }}
              项</span
            >
          </div>
          <VChart class="chart" :option="taskChart" autoresize />
        </div>
        <div class="panel">
          <div class="panel-head">
            <div>
              <h3>累计成本趋势</h3>
              <p>
                {{
                  data.cost_trend.length
                    ? `当前累计 ${money(data.cost_trend[data.cost_trend.length - 1].value)}`
                    : "暂无成本记录"
                }}
              </p>
            </div>
          </div>
          <VChart class="chart" :option="costChart" autoresize />
        </div>
      </div>
      <div v-if="data" class="grid two">
        <div class="panel">
          <div class="panel-head">
            <div>
              <h3>团队负载</h3>
              <p>周期内成员工时分配</p>
            </div>
            <router-link to="/team">查看全部</router-link>
          </div>
          <el-empty
            v-if="!data.member_loads?.length"
            description="暂无负载数据"
          />
          <div
            v-for="m in data.member_loads?.slice(0, 5)"
            :key="m.id"
            class="load-row"
          >
            <div>
              <strong>{{ m.name }}</strong
              ><span>{{ m.active_tasks }} 项进行中</span>
            </div>
            <el-progress
              :percentage="Math.min(m.load_rate, 100)"
              :status="m.load_rate > 100 ? 'exception' : undefined"
            /><b>{{ m.load_rate.toFixed(0) }}%</b>
          </div>
        </div>
        <div class="panel">
          <div class="panel-head">
            <div>
              <h3>最新风险</h3>
              <p>
                {{ data.overdue_tasks }} 项逾期 · {{ data.high_risks }} 项高风险
              </p>
            </div>
            <router-link to="/risks">进入预警中心</router-link>
          </div>
          <el-empty
            v-if="!data.latest_risks?.length"
            description="当前没有风险预警"
          />
          <div
            v-for="r in data.latest_risks?.slice(0, 5)"
            :key="r.id"
            class="risk-row"
          >
            <span class="risk-dot" :class="r.level" />
            <div>
              <strong>{{ r.title }}</strong>
              <p>{{ r.reason }}</p>
            </div>
            <el-tag
              :type="
                r.level === 'high'
                  ? 'danger'
                  : r.level === 'medium'
                    ? 'warning'
                    : 'info'
              "
              >{{
                r.level === "high" ? "高" : r.level === "medium" ? "中" : "低"
              }}</el-tag
            >
          </div>
        </div>
      </div>
      <div v-if="data" class="panel milestone-panel">
        <div class="panel-head">
          <div>
            <h3>近期里程碑</h3>
            <p>即将到期的阶段性交付目标</p>
          </div>
        </div>
        <el-empty
          v-if="!data.milestones?.length"
          description="暂无近期里程碑"
          :image-size="58"
        />
        <div v-else class="milestone-strip">
          <div v-for="m in data.milestones" :key="m.id">
            <span
              class="status-dot"
              :class="m.status === 'completed' ? 'done' : 'in_progress'"
            />
            <div>
              <b>{{ m.name }}</b
              ><small>{{ m.due_date }} · {{ m.status }}</small>
            </div>
          </div>
        </div>
      </div>
    </PageState>
  </div>
</template>
