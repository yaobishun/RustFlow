<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useRoute } from "vue-router";
import { ElMessage, ElMessageBox } from "element-plus";
import PageHeader from "../components/PageHeader.vue";
import PageState from "../components/PageState.vue";
import { projectApi, taskApi } from "../api";
import { errorMessage } from "../api/http";
import { useAuthStore } from "../stores/auth";
import type { Comment, MemberLoad, Project, Task } from "../types";

type TaskFull = Task & {
  reviews?: Array<{
    id: number;
    reviewer_name: string;
    result: string;
    comment: string;
    created_at: string;
  }>;
  activity_logs?: Array<{
    id: number;
    actor_name?: string;
    action: string;
    detail: string;
    created_at: string;
  }>;
};
const id = Number(useRoute().params.id);
const task = ref<TaskFull>();
const project = ref<Project>();
const members = ref<MemberLoad[]>([]);
const archived = computed(() => project.value?.status === "archived");
const comments = ref<Comment[]>([]);
const dependencies = ref<Task[]>([]);
const auth = useAuthStore();
const isManager = computed(() =>
  ["admin", "manager"].includes(auth.user?.role || ""),
);
const canDevelop = computed(
  () =>
    isManager.value ||
    (auth.user?.role === "developer" &&
      (task.value?.assignee_id === auth.user.id ||
        task.value?.participants?.some((p) => p.id === auth.user?.id))),
);
const canReview = computed(
  () => isManager.value || auth.user?.role === "reviewer",
);
const loading = ref(true);
const error = ref("");
const text = ref("");
const dependencyId = ref<number>();
const editDialog = ref(false);
const edit = reactive<any>({
  title: "",
  description: "",
  acceptance_criteria: "",
  assignee_id: undefined,
  priority: "medium",
  planned_start: "",
  planned_end: "",
  estimated_hours: 0,
  progress: 0,
  participant_ids: [],
});
async function load() {
  loading.value = true;
  error.value = "";
  try {
    const [detail, baseComments, deps] = await Promise.all([
      taskApi.get(id) as Promise<TaskFull>,
      taskApi.comments(id),
      taskApi.dependencies(id),
    ]);
    task.value = detail;
    project.value = await projectApi.get(detail.project_id);
    members.value = await projectApi.members(detail.project_id);
    dependencies.value = deps;
    comments.value = [
      ...baseComments,
      ...(detail.reviews || []).map((x) => ({
        id: 100000 + x.id,
        author_name: x.reviewer_name,
        content: `${x.result === "approved" ? "通过" : "驳回"}：${x.comment}`,
        created_at: x.created_at,
        kind: "review" as const,
      })),
      ...(detail.activity_logs || []).map((x) => ({
        id: 200000 + x.id,
        author_name: x.actor_name || "系统",
        content: `${x.action}${x.detail ? `：${x.detail}` : ""}`,
        created_at: x.created_at,
        kind: "system" as const,
      })),
    ].sort((a, b) => b.created_at.localeCompare(a.created_at));
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loading.value = false;
  }
}
async function comment() {
  if (!text.value.trim()) return;
  try {
    await taskApi.addComment(id, text.value);
    text.value = "";
    await load();
  } catch (e) {
    ElMessage.error(errorMessage(e));
  }
}
async function transition(status: string) {
  try {
    await taskApi.transition(id, status);
    ElMessage.success("任务状态已更新");
    await load();
  } catch (e) {
    ElMessage.error(errorMessage(e));
  }
}
async function review(action: "approve" | "reject") {
  try {
    const { value } = await ElMessageBox.prompt(
      action === "approve" ? "填写评审说明" : "填写驳回原因",
      "任务评审",
      {
        inputType: "textarea",
        inputValidator: (v) => Boolean(v) || "请填写评审意见",
      },
    );
    await taskApi.review(id, action, value);
    ElMessage.success("评审结果已提交");
    await load();
  } catch (e) {
    if (e !== "cancel" && e !== "close") ElMessage.error(errorMessage(e));
  }
}
async function addDependency() {
  if (!dependencyId.value) return;
  try {
    await taskApi.addDependency(id, dependencyId.value);
    dependencyId.value = undefined;
    ElMessage.success("前置依赖已添加");
    await load();
  } catch (e) {
    ElMessage.error(errorMessage(e));
  }
}
function openEdit() {
  Object.assign(edit, task.value);
  edit.participant_ids = task.value?.participants?.map((p) => p.id) || [];
  editDialog.value = true;
}
async function saveEdit() {
  try {
    const body = isManager.value
      ? edit
      : {
          title: edit.title,
          description: edit.description,
          acceptance_criteria: edit.acceptance_criteria,
          progress: edit.progress,
        };
    await taskApi.update(id, body);
    editDialog.value = false;
    ElMessage.success("任务信息已更新");
    await load();
  } catch (e) {
    ElMessage.error(errorMessage(e));
  }
}
const label = (s: string) =>
  ({
    todo: "待处理",
    in_progress: "进行中",
    review: "待评审",
    testing: "测试中",
    done: "已完成",
  })[s] || s;
onMounted(load);
</script>
<template>
  <div>
    <PageHeader
      :title="task?.title || '任务详情'"
      description="任务执行、依赖、评论与评审记录"
    >
      <el-button v-if="canDevelop && !archived" @click="openEdit"
        >编辑任务</el-button
      >
      <el-button
        v-if="canDevelop && !archived && task?.status === 'todo'"
        type="primary"
        @click="transition('in_progress')"
        >开始任务</el-button
      >
      <el-button
        v-if="canDevelop && !archived && task?.status === 'in_progress'"
        type="primary"
        @click="transition('review')"
        >提交评审</el-button
      >
      <template
        v-if="
          canReview &&
          !archived &&
          (task?.status === 'review' || task?.status === 'testing')
        "
        ><el-button type="danger" plain @click="review('reject')"
          >驳回修改</el-button
        ><el-button type="success" @click="review('approve')">{{
          task?.status === "review" ? "评审通过" : "测试通过"
        }}</el-button></template
      >
    </PageHeader>
    <PageState :loading :error @retry="load"
      ><div v-if="task" class="grid task-detail-grid">
        <section class="panel">
          <div class="detail-head">
            <el-tag>{{ label(task.status) }}</el-tag
            ><el-tag v-if="task.blocked" type="danger">依赖阻塞</el-tag>
          </div>
          <h3>任务说明</h3>
          <p class="detail-copy">{{ task.description || "暂无任务说明" }}</p>
          <h3>验收标准</h3>
          <p class="detail-copy">
            {{ task.acceptance_criteria || "暂无验收标准" }}
          </p>
          <div class="detail-list">
            <div>
              <span>负责人</span><b>{{ task.assignee_name || "待分配" }}</b>
            </div>
            <div>
              <span>参与者</span
              ><b>{{
                task.participants
                  ?.map((p) => p.display_name || p.name || p.username)
                  .join("、") || "无"
              }}</b>
            </div>
            <div>
              <span>计划周期</span
              ><b>{{ task.planned_start }} — {{ task.planned_end }}</b>
            </div>
            <div>
              <span>预计 / 实际工时</span
              ><b
                >{{ task.estimated_hours }}h / {{ task.actual_hours || 0 }}h</b
              >
            </div>
            <div>
              <span>完成进度</span
              ><el-progress :percentage="Number(task.progress || 0)" />
            </div>
          </div>
          <div v-if="isManager && !archived" class="panel-head">
            <div>
              <h3>前置任务</h3>
              <p>Rust 后端会拒绝循环依赖</p>
            </div>
            <div>
              <el-input-number
                v-model="dependencyId"
                :min="1"
                placeholder="任务 ID"
                controls-position="right"
              /><el-button type="primary" link @click="addDependency"
                >添加</el-button
              >
            </div>
          </div>
          <el-empty
            v-if="!dependencies.length"
            description="此任务没有前置依赖"
            :image-size="62"
          />
          <div
            v-for="dep in dependencies"
            :key="dep.id"
            class="dependency-item"
            @click="$router.push(`/tasks/${dep.id}`)"
          >
            <span class="status-dot" :class="dep.status" />
            <div>
              <b>{{ dep.title }}</b
              ><span
                >{{ label(dep.status) }} ·
                {{ dep.assignee_name || "待分配" }}</span
              >
            </div>
          </div>
        </section>
        <section class="panel">
          <div class="panel-head">
            <div>
              <h3>协作记录</h3>
              <p>评论、评审与状态变化均保留痕迹</p>
            </div>
          </div>
          <div v-if="canDevelop && !archived" class="comment-box">
            <el-input
              v-model="text"
              type="textarea"
              :rows="3"
              placeholder="补充进展、问题或协作信息…"
            /><el-button
              type="primary"
              :disabled="!text.trim()"
              @click="comment"
              >发表评论</el-button
            >
          </div>
          <el-empty
            v-if="!comments.length"
            description="暂无协作记录"
          /><el-timeline v-else
            ><el-timeline-item
              v-for="c in comments"
              :key="c.id"
              :timestamp="c.created_at"
              ><div class="comment">
                <b>{{ c.author_name }}</b
                ><el-tag size="small" effect="plain">{{
                  c.kind === "review"
                    ? "评审"
                    : c.kind === "system"
                      ? "系统"
                      : "评论"
                }}</el-tag>
                <p>{{ c.content }}</p>
              </div></el-timeline-item
            ></el-timeline
          >
        </section>
      </div></PageState
    >
    <el-dialog v-model="editDialog" title="编辑任务" width="min(620px,94vw)"
      ><el-form label-position="top"
        ><el-form-item label="任务标题"
          ><el-input v-model="edit.title" /></el-form-item
        ><el-form-item label="任务说明"
          ><el-input v-model="edit.description" type="textarea" /></el-form-item
        ><el-form-item label="验收标准"
          ><el-input v-model="edit.acceptance_criteria" type="textarea"
        /></el-form-item>
        <div class="form-grid">
          <template v-if="isManager"
            ><el-form-item label="参与者"
              ><el-select v-model="edit.participant_ids" multiple
                ><el-option
                  v-for="m in members"
                  :key="m.id"
                  :label="m.name"
                  :value="m.id" /></el-select></el-form-item
            ><el-form-item label="负责人用户 ID"
              ><el-input-number
                v-model="edit.assignee_id"
                :min="1" /></el-form-item
            ><el-form-item label="优先级"
              ><el-select v-model="edit.priority"
                ><el-option label="低" value="low" /><el-option
                  label="中"
                  value="medium" /><el-option
                  label="高"
                  value="high" /><el-option
                  label="紧急"
                  value="critical" /></el-select></el-form-item
            ><el-form-item label="计划开始"
              ><el-date-picker
                v-model="edit.planned_start"
                value-format="YYYY-MM-DD" /></el-form-item
            ><el-form-item label="计划结束"
              ><el-date-picker
                v-model="edit.planned_end"
                value-format="YYYY-MM-DD" /></el-form-item
            ><el-form-item label="预计工时"
              ><el-input-number
                v-model="edit.estimated_hours"
                :min="0"
                :step="0.5" /></el-form-item></template
          ><el-form-item label="完成进度"
            ><el-slider v-model="edit.progress" show-input
          /></el-form-item></div></el-form
      ><template #footer
        ><el-button @click="editDialog = false">取消</el-button
        ><el-button type="primary" @click="saveEdit"
          >保存修改</el-button
        ></template
      ></el-dialog
    >
  </div>
</template>
