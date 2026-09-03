import { defineStore } from "pinia";
import { ref } from "vue";
import { projectApi } from "../api";
import type { Project } from "../types";

export const useProjectStore = defineStore("projects", () => {
  const projects = ref<Project[]>([]);
  const selectedId = ref<number | undefined>(
    Number(localStorage.getItem("rustflow_project_id")) || undefined,
  );
  async function load() {
    projects.value = await projectApi.list();
    if (!projects.value.some((p) => p.id === selectedId.value))
      selectedId.value = projects.value[0]?.id;
    if (selectedId.value)
      localStorage.setItem("rustflow_project_id", String(selectedId.value));
  }
  function select(id: number) {
    selectedId.value = id;
    localStorage.setItem("rustflow_project_id", String(id));
  }
  return { projects, selectedId, load, select };
});
