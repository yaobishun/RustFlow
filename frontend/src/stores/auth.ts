import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { authApi } from "../api";
import type { User } from "../types";

export const useAuthStore = defineStore("auth", () => {
  const token = ref(localStorage.getItem("rustflow_token"));
  const cached = localStorage.getItem("rustflow_user");
  const user = ref<User | null>(cached ? JSON.parse(cached) : null);
  const authenticated = computed(() => Boolean(token.value));
  async function login(username: string, password: string) {
    const result = await authApi.login({ username, password });
    token.value = result.token;
    user.value = result.user;
    localStorage.setItem("rustflow_token", result.token);
    localStorage.setItem("rustflow_user", JSON.stringify(result.user));
  }
  async function logout() {
    try {
      if (token.value) await authApi.logout();
    } finally {
      token.value = null;
      user.value = null;
      localStorage.removeItem("rustflow_token");
      localStorage.removeItem("rustflow_user");
    }
  }
  return { token, user, authenticated, login, logout };
});
