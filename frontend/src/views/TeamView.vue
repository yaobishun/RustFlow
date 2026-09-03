<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { ElMessage } from "element-plus";
import PageHeader from "../components/PageHeader.vue";
import PageState from "../components/PageState.vue";
import { peopleApi, projectApi } from "../api";
import { errorMessage } from "../api/http";
import { useProjectStore } from "../stores/projects";
import { useAuthStore } from "../stores/auth";
import type { MemberLoad, Team, User } from "../types";
const projects = useProjectStore();
const auth = useAuthStore();
const canManage = computed(() =>
  ["admin", "manager"].includes(auth.user?.role || ""),
);
const canAdmin = computed(() => auth.user?.role === "admin");
const items = ref<MemberLoad[]>([]);
const users = ref<User[]>([]);
const teams = ref<Team[]>([]);
const loading = ref(true);
const error = ref("");
const tab = ref("load");
const dialog = ref("");
const userForm = reactive({
  username: "",
  password: "",
  display_name: "",
  role: "developer",
  hourly_rate: 60,
});
const teamForm = reactive({ name: "", description: "" });
const memberForm = reactive({
  team_id: undefined as number | undefined,
  user_id: undefined as number | undefined,
  role: "developer",
  weekly_capacity_hours: 40,
});
const editUser = reactive({
  id: 0,
  display_name: "",
  hourly_rate: 0,
  active: true,
});
function openUserEdit(u: User) {
  Object.assign(editUser, {
    id: u.id,
    display_name: u.display_name || u.name || "",
    hourly_rate: u.hourly_rate || 0,
    active: u.active !== false,
  });
  dialog.value = "editUser";
}
async function load() {
  loading.value = true;
  error.value = "";
  try {
    if (!projects.selectedId) await projects.load();
    const calls: Promise<any>[] = [peopleApi.users(), peopleApi.teams()];
    if (projects.selectedId)
      calls.push(projectApi.members(projects.selectedId));
    const [u, t, m = []] = await Promise.all(calls);
    users.value = u;
    teams.value = t;
    items.value = m;
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loading.value = false;
  }
}
async function save() {
  try {
    if (dialog.value === "user") await peopleApi.createUser(userForm);
    else if (dialog.value === "editUser")
      await peopleApi.updateUser(
        editUser.id,
        canAdmin.value ? editUser : { display_name: editUser.display_name },
      );
    else if (dialog.value === "team") await peopleApi.createTeam(teamForm);
    else if (memberForm.team_id)
      await peopleApi.addTeamMember(memberForm.team_id, memberForm);
    dialog.value = "";
    ElMessage.success("保存成功");
    await load();
  } catch (e) {
    ElMessage.error(errorMessage(e));
  }
}
const avg = computed(() =>
  items.value.length
    ? items.value.reduce((s, x) => s + x.load_rate, 0) / items.value.length
    : 0,
);
const role = (r: string) =>
  ({
    admin: "系统管理员",
    manager: "项目经理",
    developer: "开发人员",
    reviewer: "测试/评审",
  })[r] || r;
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
      title="团队与成员"
      description="管理企业角色，并平衡成员可用工时与研发任务"
      ><el-button v-if="canManage" @click="dialog = 'team'">创建团队</el-button
      ><el-button v-if="canManage" @click="dialog = 'member'"
        >添加团队成员</el-button
      ><el-button v-if="canAdmin" type="primary" @click="dialog = 'user'"
        >创建用户</el-button
      ></PageHeader
    ><PageState :loading :error @retry="load"
      ><el-tabs v-model="tab" class="team-tabs">
        <el-tab-pane label="项目负载" name="load"
          ><div class="metric-grid compact">
            <div class="simple-stat">
              <span>项目成员</span><b>{{ items.length }} 人</b>
            </div>
            <div class="simple-stat">
              <span>平均负载率</span><b>{{ avg.toFixed(0) }}%</b>
            </div>
            <div class="simple-stat">
              <span>过载成员</span
              ><b>{{ items.filter((x) => x.load_rate > 100).length }} 人</b>
            </div>
            <div class="simple-stat">
              <span>进行中任务</span
              ><b>{{ items.reduce((s, x) => s + x.active_tasks, 0) }} 项</b>
            </div>
          </div>
          <div class="panel table-panel">
            <el-table :data="items" empty-text="当前项目暂无成员"
              ><el-table-column label="成员" min-width="150"
                ><template #default="{ row }"
                  ><div class="person-cell">
                    <span class="avatar small">{{ row.name.slice(0, 1) }}</span>
                    <div>
                      <b>{{ row.name }}</b
                      ><span>{{ role(row.role) }}</span>
                    </div>
                  </div></template
                ></el-table-column
              ><el-table-column
                label="活跃任务"
                prop="active_tasks"
              /><el-table-column label="已分配 / 可用工时" min-width="160"
                ><template #default="{ row }"
                  >{{ row.assigned_hours }}h /
                  {{ row.available_hours }}h</template
                ></el-table-column
              ><el-table-column label="岗位时薪"
                ><template #default="{ row }"
                  >¥{{ row.hourly_rate }}/h</template
                ></el-table-column
              ><el-table-column label="负载率" min-width="240"
                ><template #default="{ row }"
                  ><div class="table-progress">
                    <el-progress
                      :percentage="Math.min(row.load_rate, 100)"
                      :status="
                        row.load_rate > 100
                          ? 'exception'
                          : row.load_rate > 85
                            ? 'warning'
                            : 'success'
                      "
                    /><b :class="{ danger: row.load_rate > 100 }"
                      >{{ row.load_rate.toFixed(0) }}%</b
                    >
                  </div></template
                ></el-table-column
              ></el-table
            >
          </div></el-tab-pane
        >
        <el-tab-pane label="用户与权限" name="users"
          ><div class="panel table-panel">
            <el-table :data="users" empty-text="暂无用户"
              ><el-table-column prop="username" label="账号" /><el-table-column
                prop="display_name"
                label="姓名"
              /><el-table-column label="系统角色"
                ><template #default="{ row }"
                  ><el-tag>{{ role(row.role) }}</el-tag></template
                ></el-table-column
              ><el-table-column label="岗位时薪"
                ><template #default="{ row }"
                  >¥{{ row.hourly_rate ?? 0 }}/h</template
                ></el-table-column
              ><el-table-column label="状态"
                ><template #default="{ row }"
                  ><el-tag :type="row.active === false ? 'info' : 'success'">{{
                    row.active === false ? "停用" : "正常"
                  }}</el-tag></template
                ></el-table-column
              ><el-table-column label="操作"
                ><template #default="{ row }"
                  ><el-button
                    v-if="canAdmin || row.id === auth.user?.id"
                    link
                    type="primary"
                    @click="openUserEdit(row)"
                    >编辑</el-button
                  ></template
                ></el-table-column
              ></el-table
            >
          </div></el-tab-pane
        >
        <el-tab-pane label="研发团队" name="teams"
          ><div class="project-grid">
            <article v-for="t in teams" :key="t.id" class="project-card">
              <div class="project-top">
                <el-tag effect="plain">团队 #{{ t.id }}</el-tag
                ><span>{{ t.member_count }} 名成员</span>
              </div>
              <h3>{{ t.name }}</h3>
              <p>{{ t.description || "暂无团队说明" }}</p>
              <div class="project-meta">
                <span
                  >负责人<b>{{ t.owner_name }}</b></span
                >
              </div>
            </article>
          </div></el-tab-pane
        >
      </el-tabs></PageState
    >
    <el-dialog
      v-model="dialog"
      :title="
        dialog === 'user'
          ? '创建用户'
          : dialog === 'editUser'
            ? '编辑用户资料'
            : dialog === 'team'
              ? '创建团队'
              : '添加团队成员'
      "
      width="min(520px,92vw)"
      ><el-form v-if="dialog === 'editUser'" label-position="top"
        ><el-form-item label="显示姓名"
          ><el-input v-model="editUser.display_name" /></el-form-item
        ><template v-if="canAdmin"
          ><el-form-item label="岗位时薪（元）"
            ><el-input-number
              v-model="editUser.hourly_rate"
              :min="0" /></el-form-item
          ><el-form-item label="启用状态"
            ><el-switch
              v-model="editUser.active" /></el-form-item></template></el-form
      ><el-form v-else-if="dialog === 'user'" label-position="top"
        ><div class="form-grid">
          <el-form-item label="登录账号"
            ><el-input v-model="userForm.username" /></el-form-item
          ><el-form-item label="显示姓名"
            ><el-input v-model="userForm.display_name"
          /></el-form-item>
        </div>
        <el-form-item label="初始密码"
          ><el-input v-model="userForm.password" type="password" show-password
        /></el-form-item>
        <div class="form-grid">
          <el-form-item label="系统角色"
            ><el-select v-model="userForm.role"
              ><el-option label="系统管理员" value="admin" /><el-option
                label="项目经理"
                value="manager" /><el-option
                label="开发人员"
                value="developer" /><el-option
                label="测试/评审"
                value="reviewer" /></el-select></el-form-item
          ><el-form-item label="岗位时薪（元）"
            ><el-input-number v-model="userForm.hourly_rate" :min="0"
          /></el-form-item></div></el-form
      ><el-form v-else-if="dialog === 'team'" label-position="top"
        ><el-form-item label="团队名称"
          ><el-input v-model="teamForm.name" /></el-form-item
        ><el-form-item label="团队说明"
          ><el-input
            v-model="teamForm.description"
            type="textarea" /></el-form-item></el-form
      ><el-form v-else label-position="top"
        ><el-form-item label="团队"
          ><el-select v-model="memberForm.team_id"
            ><el-option
              v-for="t in teams"
              :key="t.id"
              :label="t.name"
              :value="t.id" /></el-select></el-form-item
        ><el-form-item label="用户"
          ><el-select v-model="memberForm.user_id"
            ><el-option
              v-for="u in users"
              :key="u.id"
              :label="u.display_name || u.username"
              :value="u.id" /></el-select
        ></el-form-item>
        <div class="form-grid">
          <el-form-item label="团队角色"
            ><el-select v-model="memberForm.role"
              ><el-option label="项目经理" value="manager" /><el-option
                label="开发人员"
                value="developer" /><el-option
                label="测试/评审"
                value="reviewer" /></el-select></el-form-item
          ><el-form-item label="每周可用工时"
            ><el-input-number
              v-model="memberForm.weekly_capacity_hours"
              :min="1"
              :max="168"
          /></el-form-item></div></el-form
      ><template #footer
        ><el-button @click="dialog = ''">取消</el-button
        ><el-button type="primary" @click="save">保存</el-button></template
      ></el-dialog
    >
  </div>
</template>
