<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useRoute } from "vue-router";
import { ElMessage } from "element-plus";
import PageHeader from "../components/PageHeader.vue";
import PageState from "../components/PageState.vue";
import { projectApi, taskApi } from "../api";
import { errorMessage } from "../api/http";
import { useAuthStore } from "../stores/auth";
import type {
  MemberLoad,
  Milestone,
  Project,
  Task,
  TaskStatus,
} from "../types";
const auth = useAuthStore();
const canManage = computed(() =>
  ["admin", "manager"].includes(auth.user?.role || ""),
);
const id = Number(useRoute().params.id);
const tasks = ref<Task[]>([]);
const members = ref<MemberLoad[]>([]);
const milestones = ref<Milestone[]>([]);
const project = ref<Project>();
const loading = ref(true);
const error = ref("");
const dialog = ref(false);
const saving = ref(false);
const form = reactive<any>({
  project_id: id,
  title: "",
  priority: "medium",
  estimated_hours: 8,
  planned_start: "",
  planned_end: "",
  assignee_id: undefined,
  milestone_id: undefined,
  participant_ids: [],
});
const columns: Array<{ key: TaskStatus; label: string }> = [
  { key: "todo", label: "待处理" },
  { key: "in_progress", label: "进行中" },
  { key: "review", label: "待评审" },
  { key: "testing", label: "测试中" },
  { key: "done", label: "已完成" },
];
const grouped = computed(() =>
  Object.fromEntries(
    columns.map((c) => [c.key, tasks.value.filter((t) => t.status === c.key)]),
  ),
);
async function load() {
  loading.value = true;
  error.value = "";
  try {
    [tasks.value, members.value, milestones.value, project.value] =
      await Promise.all([
        taskApi.list(id),
        projectApi.members(id),
        projectApi.milestones(id),
        projectApi.get(id),
      ]);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loading.value = false;
  }
}
async function create() {
  saving.value = true;
  try {
    await taskApi.create(form);
    dialog.value = false;
    ElMessage.success("任务已创建");
    await load();
  } catch (e) {
    ElMessage.error(errorMessage(e));
  } finally {
    saving.value = false;
  }
}
const priorityLabel = (p: string) =>
  ({ low: "低", medium: "中", high: "高", critical: "紧急" })[p] || p;
onMounted(load);
</script>
<template>
  <div>
    <PageHeader title="任务看板" description="研发、评审、测试全过程可追踪"
      ><el-button @click="$router.push(`/projects/${id}`)">项目详情</el-button
      ><el-button
        v-if="canManage && project?.status !== 'archived'"
        type="primary"
        @click="dialog = true"
        >新建任务</el-button
      ></PageHeader
    ><PageState :loading :error @retry="load"
      ><div class="board">
        <section v-for="col in columns" :key="col.key" class="board-column">
          <header>
            <div>
              <span class="status-dot" :class="col.key" /><b>{{ col.label }}</b>
            </div>
            <em>{{ grouped[col.key]?.length || 0 }}</em>
          </header>
          <div class="card-stack">
            <article
              v-for="task in grouped[col.key]"
              :key="task.id"
              class="task-card"
              @click="$router.push(`/tasks/${task.id}`)"
            >
              <div class="task-tags">
                <el-tag
                  size="small"
                  :type="
                    task.priority === 'critical'
                      ? 'danger'
                      : task.priority === 'high'
                        ? 'warning'
                        : 'info'
                  "
                  >{{ priorityLabel(task.priority) }}</el-tag
                ><span v-if="task.blocked" class="blocked">依赖阻塞</span>
              </div>
              <h4>{{ task.title }}</h4>
              <p>{{ task.description || "暂无任务说明" }}</p>
              <el-progress
                :percentage="Number(task.progress || 0)"
                :show-text="false"
              />
              <div class="task-foot">
                <span class="mini-avatar">{{
                  (task.assignee_name || "?").slice(0, 1)
                }}</span
                ><span>{{ task.assignee_name || "待分配" }}</span
                ><time>{{ task.planned_end || "未设日期" }}</time>
              </div>
            </article>
            <div v-if="!grouped[col.key]?.length" class="column-empty">
              暂无任务
            </div>
          </div>
        </section>
      </div></PageState
    >
    <el-dialog v-model="dialog" title="新建研发任务" width="min(620px,94vw)"
      ><el-form label-position="top"
        ><el-form-item label="任务标题"
          ><el-input v-model="form.title" /></el-form-item
        ><el-form-item label="任务说明"
          ><el-input
            v-model="form.description"
            type="textarea"
            :rows="3" /></el-form-item
        ><el-form-item label="验收标准"
          ><el-input v-model="form.acceptance_criteria" type="textarea"
        /></el-form-item>
        <div class="form-grid">
          <el-form-item label="负责人"
            ><el-select
              v-model="form.assignee_id"
              clearable
              placeholder="可稍后分配"
              ><el-option
                v-for="m in members"
                :key="m.id"
                :label="`${m.name}（负载 ${m.load_rate.toFixed(0)}%）`"
                :value="m.id" /></el-select></el-form-item
          ><el-form-item label="所属里程碑"
            ><el-select
              v-model="form.milestone_id"
              clearable
              placeholder="选择里程碑"
              ><el-option
                v-for="m in milestones"
                :key="m.id"
                :label="m.name"
                :value="m.id" /></el-select></el-form-item
          ><el-form-item label="参与者"
            ><el-select
              v-model="form.participant_ids"
              multiple
              clearable
              placeholder="选择协作成员"
              ><el-option
                v-for="m in members"
                :key="m.id"
                :label="m.name"
                :value="m.id" /></el-select></el-form-item
          ><el-form-item label="优先级"
            ><el-select v-model="form.priority"
              ><el-option label="低" value="low" /><el-option
                label="中"
                value="medium" /><el-option label="高" value="high" /><el-option
                label="紧急"
                value="critical" /></el-select></el-form-item
          ><el-form-item label="预计工时"
            ><el-input-number
              v-model="form.estimated_hours"
              :min="0.5"
              :step="0.5" /></el-form-item
          ><el-form-item label="计划开始"
            ><el-date-picker
              v-model="form.planned_start"
              value-format="YYYY-MM-DD" /></el-form-item
          ><el-form-item label="计划结束"
            ><el-date-picker
              v-model="form.planned_end"
              value-format="YYYY-MM-DD"
          /></el-form-item></div></el-form
      ><template #footer
        ><el-button @click="dialog = false">取消</el-button
        ><el-button
          type="primary"
          :loading="saving"
          :disabled="!form.title || !form.planned_start || !form.planned_end"
          @click="create"
          >创建任务</el-button
        ></template
      ></el-dialog
    >
  </div>
</template>
