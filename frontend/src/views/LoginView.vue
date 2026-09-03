<script setup lang="ts">
import { reactive, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { ElMessage } from "element-plus";
import { useAuthStore } from "../stores/auth";
import { errorMessage } from "../api/http";

const form = reactive({ username: "", password: "" });
const loading = ref(false);
const auth = useAuthStore();
const router = useRouter();
const route = useRoute();
async function submit() {
  if (!form.username || !form.password)
    return ElMessage.warning("请输入用户名和密码");
  loading.value = true;
  try {
    await auth.login(form.username, form.password);
    router.replace(String(route.query.redirect || "/dashboard"));
  } catch (e) {
    ElMessage.error(errorMessage(e));
  } finally {
    loading.value = false;
  }
}
</script>
<template>
  <main class="login-page">
    <section class="login-story">
      <div class="brand large">
        <div class="brand-mark">R</div>
        <div class="brand-copy">
          <strong>RustFlow</strong><span>企业研发项目协同与成本决策平台</span>
        </div>
      </div>
      <div class="story-copy">
        <span class="eyebrow">RUST-POWERED PROJECT INTELLIGENCE</span>
        <h1>让研发协作、工程进度与经济决策形成闭环。</h1>
        <p>
          从任务依赖到挣值分析，从成本追踪到方案评价，所有关键业务规则均由 Rust
          确定性计算。
        </p>
      </div>
      <div class="feature-row">
        <span>任务协同</span><span>风险传播</span><span>EVM 绩效</span
        ><span>经济决策</span>
      </div>
    </section>
    <section class="login-panel">
      <div class="login-box">
        <span class="eyebrow">欢迎回来</span>
        <h2>登录工作台</h2>
        <p>使用企业账户访问您的研发项目。</p>
        <el-form label-position="top" @keyup.enter="submit"
          ><el-form-item label="用户名"
            ><el-input
              v-model="form.username"
              size="large"
              placeholder="请输入用户名" /></el-form-item
          ><el-form-item label="密码"
            ><el-input
              v-model="form.password"
              size="large"
              type="password"
              show-password
              placeholder="请输入密码" /></el-form-item
          ><el-button
            type="primary"
            size="large"
            :loading="loading"
            class="login-button"
            @click="submit"
            >登录 RustFlow</el-button
          ></el-form
        >
        <div class="connection-note">
          需连接 RustFlow 后端服务 · 首次启动自动初始化演示项目
        </div>
      </div>
    </section>
  </main>
</template>
