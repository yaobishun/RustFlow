<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ElMessage } from "element-plus";
import PageHeader from "../components/PageHeader.vue";
import PageState from "../components/PageState.vue";
import { riskApi } from "../api";
import { errorMessage } from "../api/http";
import type { Risk } from "../types";
import { useProjectStore } from "../stores/projects";
import { useAuthStore } from "../stores/auth";
const projects = useProjectStore();
const auth = useAuthStore();
const canManage = computed(() =>
  ["admin", "manager"].includes(auth.user?.role || ""),
);
const archived = computed(
  () =>
    projects.projects.find((p) => p.id === projects.selectedId)?.status ===
    "archived",
);
const items = ref<Risk[]>([]);
const loading = ref(true);
const scanning = ref(false);
const error = ref("");
const filter = ref("all");
async function projectId() {
  if (!projects.selectedId) await projects.load();
  if (!projects.selectedId) throw new Error("暂无可用项目，请先创建项目");
  return projects.selectedId;
}
async function load() {
  loading.value = true;
  error.value = "";
  try {
    items.value = await riskApi.list(await projectId());
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loading.value = false;
  }
}
async function scan() {
  scanning.value = true;
  try {
    items.value = await riskApi.rescan(await projectId());
    ElMessage.success("风险扫描完成");
  } catch (e) {
    ElMessage.error(errorMessage(e));
  } finally {
    scanning.value = false;
  }
}
const shown = computed(() =>
  filter.value === "all"
    ? items.value
    : items.value.filter((x) => x.level === filter.value),
);
onMounted(() => {
  load();
  window.addEventListener("rustflow:project-change", load);
});
</script>
<template>
  <div>
    <PageHeader
      title="风险预警"
      description="识别延期传播、依赖阻塞、成员过载和成本偏差"
      ><el-button
        v-if="canManage && !archived"
        type="primary"
        :loading="scanning"
        @click="scan"
        >重新扫描</el-button
      ></PageHeader
    >
    <div class="risk-summary">
      <button :class="{ active: filter === 'all' }" @click="filter = 'all'">
        <span>全部风险</span><b>{{ items.length }}</b></button
      ><button :class="{ active: filter === 'high' }" @click="filter = 'high'">
        <span>高风险</span
        ><b class="danger">{{
          items.filter((x) => x.level === "high").length
        }}</b></button
      ><button
        :class="{ active: filter === 'medium' }"
        @click="filter = 'medium'"
      >
        <span>中风险</span
        ><b class="warning">{{
          items.filter((x) => x.level === "medium").length
        }}</b></button
      ><button :class="{ active: filter === 'low' }" @click="filter = 'low'">
        <span>低风险</span
        ><b>{{ items.filter((x) => x.level === "low").length }}</b>
      </button>
    </div>
    <PageState
      :loading
      :error
      :empty="!shown.length"
      empty-text="当前筛选下没有风险预警"
      @retry="load"
      ><div class="risk-list">
        <article v-for="r in shown" :key="r.id" class="risk-card">
          <div class="risk-icon" :class="r.level">!</div>
          <div class="risk-main">
            <div>
              <el-tag
                :type="
                  r.level === 'high'
                    ? 'danger'
                    : r.level === 'medium'
                      ? 'warning'
                      : 'info'
                "
                >{{
                  r.level === "high"
                    ? "高风险"
                    : r.level === "medium"
                      ? "中风险"
                      : "低风险"
                }}</el-tag
              ><span>{{ r.type }}</span
              ><time>{{ r.created_at }}</time>
            </div>
            <h3>{{ r.title }}</h3>
            <p>{{ r.reason }}</p>
            <small v-if="r.related_task">关联任务：{{ r.related_task }}</small>
            <div v-if="r.affected_tasks?.length" class="affected-list">
              <b>受影响任务：</b
              ><span v-for="t in r.affected_tasks" :key="t.id"
                >{{ t.title }}（{{ t.assignee_name || "待分配" }}）</span
              >
            </div>
          </div>
          <el-tag :type="r.resolved ? 'success' : 'danger'" effect="plain">{{
            r.resolved ? "已解除" : "待处理"
          }}</el-tag>
        </article>
      </div></PageState
    >
  </div>
</template>
