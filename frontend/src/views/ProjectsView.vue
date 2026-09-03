<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { ElMessage } from "element-plus";
import PageHeader from "../components/PageHeader.vue";
import PageState from "../components/PageState.vue";
import { peopleApi, projectApi } from "../api";
import { errorMessage } from "../api/http";
import { useAuthStore } from "../stores/auth";
import type { Project, Team, User } from "../types";
const auth = useAuthStore(),
  canManage = computed(() =>
    ["admin", "manager"].includes(auth.user?.role || ""),
  );
const items = ref<Project[]>([]),
  teams = ref<Team[]>([]),
  users = ref<User[]>([]),
  loading = ref(true),
  error = ref(""),
  dialog = ref(false),
  saving = ref(false);
const form = reactive({
  team_id: undefined as number | undefined,
  manager_id: undefined as number | undefined,
  name: "",
  description: "",
  start_date: "",
  end_date: "",
  budget: 0,
});
async function load() {
  loading.value = true;
  error.value = "";
  try {
    const [p, t, u] = await Promise.all([
      projectApi.list(),
      peopleApi.teams(),
      peopleApi.users(),
    ]);
    items.value = p;
    teams.value = t;
    users.value = u.filter((x) => ["admin", "manager"].includes(x.role));
    if (!form.manager_id) form.manager_id = auth.user?.id;
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loading.value = false;
  }
}
async function create() {
  saving.value = true;
  try {
    await projectApi.create(form);
    dialog.value = false;
    ElMessage.success("项目创建成功");
    await load();
  } catch (e) {
    ElMessage.error(errorMessage(e));
  } finally {
    saving.value = false;
  }
}
const status = (s: string) =>
  ({
    active: "进行中",
    planning: "规划中",
    completed: "已完成",
    archived: "已归档",
  })[s] || s;
onMounted(load);
</script>
<template>
  <div>
    <PageHeader
      title="项目管理"
      description="按研发阶段组织范围、预算、里程碑与成员"
      ><el-button v-if="canManage" type="primary" @click="dialog = true"
        >创建项目</el-button
      ></PageHeader
    ><PageState
      :loading
      :error
      :empty="!items.length"
      empty-text="尚未创建项目"
      @retry="load"
      ><div class="project-grid">
        <article
          v-for="p in items"
          :key="p.id"
          class="project-card"
          @click="$router.push(`/projects/${p.id}`)"
        >
          <div class="project-top">
            <el-tag effect="plain">{{ status(p.status) }}</el-tag
            ><span>{{ p.start_date }} — {{ p.end_date }}</span>
          </div>
          <h3>{{ p.name }}</h3>
          <p>{{ p.description || "暂无项目说明" }}</p>
          <div class="project-meta">
            <span
              >负责人<b>{{ p.manager_name || "未指定" }}</b></span
            ><span
              >预算<b>¥{{ Number(p.budget).toLocaleString() }}</b></span
            >
          </div>
          <el-progress :percentage="Number(p.completion_rate || 0)" /><button
            class="text-link"
          >
            查看项目详情 →
          </button>
        </article>
      </div></PageState
    ><el-dialog
      v-if="canManage"
      v-model="dialog"
      title="创建研发项目"
      width="min(560px,92vw)"
      ><el-form label-position="top"
        ><div class="form-grid">
          <el-form-item label="所属团队"
            ><el-select v-model="form.team_id"
              ><el-option
                v-for="t in teams"
                :key="t.id"
                :label="t.name"
                :value="t.id" /></el-select></el-form-item
          ><el-form-item label="项目经理"
            ><el-select v-model="form.manager_id"
              ><el-option
                v-for="u in users"
                :key="u.id"
                :label="u.display_name || u.username"
                :value="u.id" /></el-select
          ></el-form-item>
        </div>
        <el-form-item label="项目名称"
          ><el-input v-model="form.name" /></el-form-item
        ><el-form-item label="项目说明"
          ><el-input v-model="form.description" type="textarea"
        /></el-form-item>
        <div class="form-grid">
          <el-form-item label="开始日期"
            ><el-date-picker
              v-model="form.start_date"
              value-format="YYYY-MM-DD" /></el-form-item
          ><el-form-item label="结束日期"
            ><el-date-picker v-model="form.end_date" value-format="YYYY-MM-DD"
          /></el-form-item>
        </div>
        <el-form-item label="总预算（元）"
          ><el-input-number
            v-model="form.budget"
            :min="0" /></el-form-item></el-form
      ><template #footer
        ><el-button @click="dialog = false">取消</el-button
        ><el-button
          type="primary"
          :loading="saving"
          :disabled="
            !form.team_id ||
            !form.manager_id ||
            !form.name ||
            !form.start_date ||
            !form.end_date
          "
          @click="create"
          >创建</el-button
        ></template
      ></el-dialog
    >
  </div>
</template>
