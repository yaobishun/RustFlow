import axios, { AxiosError } from "axios";

const defaultApiBase =
  location.protocol === "file:" ? "http://127.0.0.1:3100/api" : "/api";

export const http = axios.create({
  baseURL: import.meta.env.VITE_API_BASE || defaultApiBase,
  timeout: 12000,
});

http.interceptors.request.use((config) => {
  const token = localStorage.getItem("rustflow_token");
  if (token) config.headers.Authorization = `Bearer ${token}`;
  return config;
});

http.interceptors.response.use(
  (response) => response,
  (error: AxiosError<{ message?: string; error?: string }>) => {
    if (error.response?.status === 401) {
      localStorage.removeItem("rustflow_token");
      localStorage.removeItem("rustflow_user");
      if (location.pathname !== "/login") location.href = "/login";
    }
    return Promise.reject(error);
  },
);

export function unwrap<T>(response: { data: T | { data: T } }): T {
  const body = response.data;
  return typeof body === "object" && body !== null && "data" in body
    ? (body as { data: T }).data
    : (body as T);
}

export function errorMessage(error: unknown): string {
  if (axios.isAxiosError(error)) {
    if (!error.response)
      return "无法连接后端服务，请确认 Rust 服务已在 3000 端口启动。";
    return (
      error.response.data?.message ||
      error.response.data?.error ||
      `请求失败（HTTP ${error.response.status}）`
    );
  }
  return error instanceof Error ? error.message : "发生未知错误";
}
