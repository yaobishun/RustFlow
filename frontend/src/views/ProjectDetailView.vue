<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { ElMessage, ElMessageBox } from "element-plus";
import PageHeader from "../components/PageHeader.vue";
import PageState from "../components/PageState.vue";
import { projectApi } from "../api";
import { errorMessage } from "../api/http";
import type { MemberLoad, Milestone, Project } from "../types";
import { useAuthStore } from "../stores/auth";
const route = useRoute();
const router = useRouter();
const auth = useAuthStore();
const canManage = computed(() =>
  ["admin", "manager"].includes(auth.user?.role || ""),
);
const id = Number(route.params.id);
const project = ref<Project>();
const milestones = ref<Milestone[]>([]);
const members = ref<MemberLoad[]>([]);
const loading = ref(true);
const error = ref("");
const dialog = ref("");
const dialogVisible = computed({
  get: () => dialog.value !== "",
  set: (visible: boolean) => {
    if (!visible) dialog.value = "";
  },
});
const milestone = reactive({
  name: "",
  description: "",
  start_date: "",
  end_date: "",
});
const edit = reactive<Partial<Project>>({
  name: "",
  description: "",
  start_date: "",
  end_date: "",
  budget: 0,
});
const member = reactive({
  user_id: undefined as number | undefined,
  project_role: "developer",
});
async function load() {
  loading.value = true;
  error.value = "";
  try {
    [project.value, milestones.value, members.value] = await Promise.all([
      projectApi.get(id),
      projectApi.milestones(id),
      projectApi.members(id),
    ]);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loading.value = false;
  }
}
async function createMilestone() {
  try {
    await projectApi.createMilestone(id, milestone);
    dialog.value = "";
    ElMessage.success("里程碑已添加");
    await load();
  } catch (e) {
    ElMessage.error(errorMessage(e));
  }
}
async function addMember() {
  if (!member.user_id) return;
  try {
    await projectApi.addMember(id, member);
    dialog.value = "";
    ElMessage.success("项目成员已添加");
    await load();
  } catch (e) {
    ElMessage.error(errorMessage(e));
  }
}
function openEdit() {
  Object.assign(edit, project.value);
  dialog.value = "edit";
}
async function saveEdit() {
  try {
    await projectApi.update(id, edit);
    dialog.value = "";
    ElMessage.success("项目已更新");
    await load();
  } catch (e) {
    ElMessage.error(errorMessage(e));
  }
}
async function archive() {
  try {
    await ElMessageBox.confirm(
      "归档后项目转为只读，但仍可查看历史数据，确认继续？",
      "归档项目",
      { type: "warning" },
    );
    await projectApi.archive(id);
    ElMessage.success("项目已归档");
    router.push("/projects");
  } catch (e) {
    if (e !== "cancel" && e !== "close") ElMessage.error(errorMessage(e));
  }
}
const days = computed(() =>
  project.value
    ? Math.ceil(
        (new Date(project.value.end_date).getTime() - Date.now()) / 86400000,
      )
    : 0,
);
onMounted(load);
</script>
<template>
  <div>
    <PageHeader
      :title="project?.name || '项目详情'"
      description="项目范围、阶段计划、成员与交付进展"
      ><el-button @click="$router.push(`/projects/${id}/board`)"
        >任务看板</el-button
      ><el-button
        v-if="canManage && project?.status !== 'archived'"
        @click="dialog = 'member'"
        >添加成员</el-button
      ><el-button
        v-if="canManage && project?.status !== 'archived'"
        @click="openEdit"
        >编辑项目</el-button
      ><el-button
        v-if="canManage && project?.status !== 'archived'"
        type="danger"
        plain
        @click="archive"
        >归档</el-button
      ><el-button
        v-if="canManage && project?.status !== 'archived'"
        type="primary"
        @click="dialog = 'milestone'"
        >添加里程碑</el-button
      ></PageHeader
    >
    <PageState :loading :error @retry="load"
      ><template v-if="project">
        <div class="project-hero">
          <div>
            <span class="eyebrow">PROJECT OVERVIEW</span>
            <h2>{{ project.name }}</h2>
            <p>{{ project.description || "暂无项目说明" }}</p>
          </div>
          <div class="hero-progress">
            <el-progress
              type="dashboard"
              :percentage="Number(project.completion_rate || 0)"
              :width="118"
            /><span>剩余 {{ Math.max(days, 0) }} 天</span>
          </div>
        </div>
        <div class="metric-grid compact">
          <div class="simple-stat">
            <span>项目负责人</span><b>{{ project.manager_name || "未指定" }}</b>
          </div>
          <div class="simple-stat">
            <span>项目周期</span
            ><b>{{ project.start_date }} — {{ project.end_date }}</b>
          </div>
          <div class="simple-stat">
            <span>项目总预算</span
            ><b>¥{{ Number(project.budget).toLocaleString() }}</b>
          </div>
          <div class="simple-stat">
            <span>当前状态</span><b>{{ project.status }}</b>
          </div>
        </div>
        <div class="grid two">
          <div class="panel">
            <div class="panel-head">
              <div>
                <h3>项目成员</h3>
                <p>成员角色与当前任务负载</p>
              </div>
            </div>
            <el-empty
              v-if="!members.length"
              description="尚未添加项目成员"
              :image-size="60"
            />
            <div v-for="m in members" :key="m.id" class="load-row">
              <div>
                <strong>{{ m.name }}</strong
                ><span>{{ m.role }}</span>
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
                <h3>里程碑路线</h3>
                <p>按研发生命周期跟踪阶段交付</p>
              </div>
            </div>
            <el-empty
              v-if="!milestones.length"
              description="暂无里程碑"
              :image-size="60"
            /><el-timeline v-else
              ><el-timeline-item
                v-for="m in milestones"
                :key="m.id"
                :timestamp="m.due_date"
                :type="m.completion_rate >= 100 ? 'success' : 'primary'"
                ><div class="milestone-item">
                  <div>
                    <h4>{{ m.name }}</h4>
                    <span>{{ m.status }}</span>
                  </div>
                  <el-progress
                    :percentage="Number(m.completion_rate || 0)"
                  /></div></el-timeline-item
            ></el-timeline>
          </div>
        </div> </template
    ></PageState>
    <el-dialog
      v-model="dialogVisible"
      :title="
        dialog === 'edit'
          ? '编辑项目'
          : dialog === 'member'
            ? '添加项目成员'
            : '添加里程碑'
      "
      width="min(560px,92vw)"
    >
      <el-form v-if="dialog === 'edit'" label-position="top"
        ><el-form-item label="项目名称"
          ><el-input v-model="edit.name" /></el-form-item
        ><el-form-item label="项目说明"
          ><el-input v-model="edit.description" type="textarea"
        /></el-form-item>
        <div class="form-grid">
          <el-form-item label="开始日期"
            ><el-date-picker
              v-model="edit.start_date"
              value-format="YYYY-MM-DD" /></el-form-item
          ><el-form-item label="结束日期"
            ><el-date-picker v-model="edit.end_date" value-format="YYYY-MM-DD"
          /></el-form-item>
        </div>
        <el-form-item label="总预算（元）"
          ><el-input-number
            v-model="edit.budget"
            :min="0"
            :step="1000" /></el-form-item
      ></el-form>
      <el-form v-else-if="dialog === 'member'" label-position="top"
        ><el-form-item label="用户 ID"
          ><el-input-number v-model="member.user_id" :min="1" />
          <div class="form-tip">
            请填写“团队与成员”页面中的用户 ID
          </div></el-form-item
        ><el-form-item label="项目角色"
          ><el-select v-model="member.project_role"
            ><el-option label="项目经理" value="manager" /><el-option
              label="开发人员"
              value="developer" /><el-option
              label="测试/评审"
              value="reviewer" /></el-select></el-form-item
      ></el-form>
      <el-form v-else label-position="top"
        ><el-form-item label="阶段名称"
          ><el-input v-model="milestone.name" /></el-form-item
        ><el-form-item label="阶段说明"
          ><el-input v-model="milestone.description" type="textarea"
        /></el-form-item>
        <div class="form-grid">
          <el-form-item label="计划开始"
            ><el-date-picker
              v-model="milestone.start_date"
              value-format="YYYY-MM-DD" /></el-form-item
          ><el-form-item label="计划完成"
            ><el-date-picker
              v-model="milestone.end_date"
              value-format="YYYY-MM-DD"
          /></el-form-item></div
      ></el-form>
      <template #footer
        ><el-button @click="dialog = ''">取消</el-button
        ><el-button
          type="primary"
          @click="
            dialog === 'edit'
              ? saveEdit()
              : dialog === 'member'
                ? addMember()
                : createMilestone()
          "
          >保存</el-button
        ></template
      >
    </el-dialog>
  </div>
</template>
